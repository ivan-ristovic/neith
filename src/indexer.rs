use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tantivy::schema::*;
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, Term, doc};

use crate::library::{
    EntryDoc, FileSignature, Library, SourceKind, content_hash, discover_markdown_entries,
    discover_signatures,
};

const INDEX_VERSION: u32 = 1;

#[derive(Clone)]
pub struct SearchFields {
    pub path_exact: Field,
    pub path_text: Field,
    pub rel_path: Field,
    pub library: Field,
    pub title: Field,
    pub body: Field,
    pub excerpt: Field,
    pub source_kind: Field,
    pub size: Field,
    pub modified_unix: Field,
}

#[derive(Clone)]
pub struct IndexHandle {
    pub library: Library,
    pub index: Index,
    pub reader: IndexReader,
    pub fields: SearchFields,
    pub catalog: Vec<CatalogEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub library_alias: String,
    pub path: String,
    pub rel_path: String,
    pub title: String,
    pub excerpt: String,
    pub source_kind: String,
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct IndexManifest {
    pub version: u32,
    pub files: Vec<FileSignature>,
}

#[derive(Debug, Default)]
pub struct IndexStats {
    pub indexed: usize,
    pub removed: usize,
    pub unchanged: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct LibraryStatus {
    pub alias: String,
    pub kind: String,
    pub files: usize,
    pub indexed: usize,
    pub stale: usize,
    pub cache_bytes: Option<u64>,
    pub index: String,
    pub cache_dir: PathBuf,
}

#[derive(Clone)]
pub struct IndexManager {
    pub handles: Vec<IndexHandle>,
}

impl IndexManager {
    pub fn open(libraries: &[Library]) -> Result<Self> {
        let mut handles = Vec::new();
        for library in libraries {
            let (index, fields) = open_index(library)?;
            let reader = index
                .reader_builder()
                .reload_policy(ReloadPolicy::OnCommitWithDelay)
                .try_into()
                .context("failed to open index reader")?;
            let catalog = load_catalog(library)?;
            handles.push(IndexHandle {
                library: library.clone(),
                index,
                reader,
                fields,
                catalog,
            });
        }
        Ok(Self { handles })
    }

    pub fn reload(&self) -> Result<()> {
        for handle in &self.handles {
            handle.reader.reload()?;
        }
        Ok(())
    }

    pub fn has_usable_indexes(libraries: &[Library]) -> bool {
        libraries.iter().all(|library| {
            index_dir(library).join("meta.json").is_file() && manifest_path(library).is_file()
        })
    }
}

pub fn ensure_indexes<F>(
    libraries: &[Library],
    rebuild: bool,
    mut progress: F,
) -> Result<Vec<IndexStats>>
where
    F: FnMut(&Library, &str),
{
    let mut stats = Vec::new();
    for library in libraries {
        progress(library, "scanning");
        if rebuild {
            let _ = fs::remove_dir_all(library.cache_dir());
        }
        let stat = ensure_library_index(library, |stage| progress(library, stage))?;
        stats.push(stat);
    }
    Ok(stats)
}

fn ensure_library_index<F>(library: &Library, mut progress: F) -> Result<IndexStats>
where
    F: FnMut(&str),
{
    fs::create_dir_all(library.cache_dir())?;
    let (index, fields) = open_index(library)?;
    let mut writer: IndexWriter = index.writer(100_000_000)?;

    progress("reading entries");
    let entries = discover_markdown_entries(library)?;
    let current: HashMap<String, EntryDoc> = entries
        .into_iter()
        .map(|entry| (entry.path.to_string_lossy().to_string(), entry))
        .collect();
    let previous = load_manifest(library).unwrap_or_default();
    let previous_by_path: HashMap<String, FileSignature> = previous
        .files
        .into_iter()
        .map(|signature| (signature.path.clone(), signature))
        .collect();

    let mut stats = IndexStats::default();
    let current_paths: HashSet<&str> = current.keys().map(String::as_str).collect();
    for old_path in previous_by_path.keys() {
        if !current_paths.contains(old_path.as_str()) {
            writer.delete_term(Term::from_field_text(fields.path_exact, old_path));
            stats.removed += 1;
        }
    }

    progress("indexing changed files");
    let mut next_manifest = IndexManifest {
        version: INDEX_VERSION,
        files: Vec::with_capacity(current.len()),
    };

    for (path, entry) in current {
        let signature = FileSignature {
            path: path.clone(),
            rel_path: entry.rel_path.clone(),
            title: entry.title.clone(),
            excerpt: entry.excerpt.clone(),
            source_kind: entry.source_kind,
            size: entry.size,
            modified_unix: entry.modified_unix,
            content_hash: content_hash(&entry.body),
        };
        if previous_by_path.get(&path) == Some(&signature) {
            stats.unchanged += 1;
            next_manifest.files.push(signature);
            continue;
        }
        writer.delete_term(Term::from_field_text(fields.path_exact, &path));
        writer.add_document(entry_to_tantivy_doc(&fields, &entry))?;
        stats.indexed += 1;
        next_manifest.files.push(signature);
    }

    if stats.indexed > 0 || stats.removed > 0 || !manifest_path(library).is_file() {
        progress("committing");
        writer.commit()?;
    }
    write_manifest(library, &next_manifest)?;
    write_catalog(library, &next_manifest)?;
    Ok(stats)
}

fn entry_to_tantivy_doc(fields: &SearchFields, entry: &EntryDoc) -> TantivyDocument {
    let path = entry.path.to_string_lossy();
    doc!(
        fields.path_exact => path.as_ref(),
        fields.path_text => path.as_ref(),
        fields.rel_path => entry.rel_path.as_str(),
        fields.library => entry.library_alias.as_str(),
        fields.title => entry.title.as_str(),
        fields.body => entry.body.as_str(),
        fields.excerpt => entry.excerpt.as_str(),
        fields.source_kind => entry.source_kind.as_str(),
        fields.size => entry.size,
        fields.modified_unix => entry.modified_unix,
    )
}

fn open_index(library: &Library) -> Result<(Index, SearchFields)> {
    fs::create_dir_all(index_dir(library))?;
    let schema = build_schema();
    let index = match Index::open_in_dir(index_dir(library)) {
        Ok(index) => index,
        Err(_) => {
            let _ = fs::remove_dir_all(index_dir(library));
            fs::create_dir_all(index_dir(library))?;
            Index::create_in_dir(index_dir(library), schema.clone())?
        }
    };
    let fields = fields_from_schema(&index.schema())?;
    Ok((index, fields))
}

fn build_schema() -> Schema {
    let mut schema = Schema::builder();
    schema.add_text_field("path_exact", STRING | STORED);
    schema.add_text_field("path_text", TEXT | STORED);
    schema.add_text_field("rel_path", TEXT | STORED);
    schema.add_text_field("library", STRING | STORED);
    schema.add_text_field("title", TEXT | STORED);
    schema.add_text_field("body", TEXT | STORED);
    schema.add_text_field("excerpt", TEXT | STORED);
    schema.add_text_field("source_kind", STRING | STORED);
    schema.add_u64_field("size", STORED);
    schema.add_u64_field("modified_unix", STORED);
    schema.build()
}

fn fields_from_schema(schema: &Schema) -> Result<SearchFields> {
    Ok(SearchFields {
        path_exact: schema.get_field("path_exact")?,
        path_text: schema.get_field("path_text")?,
        rel_path: schema.get_field("rel_path")?,
        library: schema.get_field("library")?,
        title: schema.get_field("title")?,
        body: schema.get_field("body")?,
        excerpt: schema.get_field("excerpt")?,
        source_kind: schema.get_field("source_kind")?,
        size: schema.get_field("size")?,
        modified_unix: schema.get_field("modified_unix")?,
    })
}

pub fn index_dir(library: &Library) -> PathBuf {
    library.cache_dir().join("tantivy")
}

pub fn manifest_path(library: &Library) -> PathBuf {
    library.cache_dir().join("manifest.json")
}

pub fn catalog_path(library: &Library) -> PathBuf {
    library.cache_dir().join("catalog.json")
}

pub fn library_status(library: &Library) -> Result<LibraryStatus> {
    let signatures = discover_signatures(library)?;
    let files = signatures.len();
    let kind = library_kind(&signatures);
    let manifest_file_exists = manifest_path(library).is_file();
    let catalog_file_exists = catalog_path(library).is_file();
    let index_meta_exists = index_dir(library).join("meta.json").is_file();
    let manifest = load_manifest(library).ok();
    let indexed = manifest.as_ref().map_or(0, |manifest| manifest.files.len());
    let stale = manifest
        .as_ref()
        .map(|manifest| stale_count(&signatures, manifest))
        .unwrap_or(files);
    let index = if !manifest_file_exists || !catalog_file_exists || !index_meta_exists {
        "missing"
    } else if stale > 0 {
        "stale"
    } else {
        "ok"
    }
    .to_string();
    let cache_dir = library.cache_dir();
    let cache_bytes = cache_dir
        .is_dir()
        .then(|| directory_size(&cache_dir))
        .transpose()?;

    Ok(LibraryStatus {
        alias: library.alias.clone(),
        kind,
        files,
        indexed,
        stale,
        cache_bytes,
        index,
        cache_dir,
    })
}

fn library_kind(signatures: &[FileSignature]) -> String {
    let mut kinds = signatures
        .iter()
        .map(|signature| signature.source_kind)
        .collect::<HashSet<SourceKind>>();
    if kinds.len() > 1 {
        return "mixed".to_string();
    }
    kinds
        .drain()
        .next()
        .map(SourceKind::as_str)
        .unwrap_or("empty")
        .to_string()
}

fn stale_count(current: &[FileSignature], manifest: &IndexManifest) -> usize {
    let manifest_by_path = manifest
        .files
        .iter()
        .map(|signature| (signature.path.as_str(), signature))
        .collect::<HashMap<_, _>>();
    let current_paths = current
        .iter()
        .map(|signature| signature.path.as_str())
        .collect::<HashSet<_>>();
    let changed = current
        .iter()
        .filter(|signature| manifest_by_path.get(signature.path.as_str()) != Some(signature))
        .count();
    let removed = manifest
        .files
        .iter()
        .filter(|signature| !current_paths.contains(signature.path.as_str()))
        .count();
    changed + removed
}

fn directory_size(path: &Path) -> Result<u64> {
    let mut size = 0;
    for item in walkdir::WalkDir::new(path).follow_links(false) {
        let item = item?;
        if item.file_type().is_file() {
            size += item.metadata()?.len();
        }
    }
    Ok(size)
}

fn load_manifest(library: &Library) -> Result<IndexManifest> {
    let text = fs::read_to_string(manifest_path(library))?;
    let manifest: IndexManifest = serde_json::from_str(&text)?;
    if manifest.version == INDEX_VERSION {
        Ok(manifest)
    } else {
        Ok(IndexManifest::default())
    }
}

fn write_manifest(library: &Library, manifest: &IndexManifest) -> Result<()> {
    write_json_atomic(&manifest_path(library), manifest)
}

fn write_catalog(library: &Library, manifest: &IndexManifest) -> Result<()> {
    let entries = manifest
        .files
        .iter()
        .map(|signature| CatalogEntry {
            library_alias: library.alias.clone(),
            path: signature.path.clone(),
            rel_path: signature.rel_path.clone(),
            title: signature.title.clone(),
            excerpt: signature.excerpt.clone(),
            source_kind: signature.source_kind.as_str().to_string(),
        })
        .collect::<Vec<_>>();
    write_json_atomic(&catalog_path(library), &entries)
}

fn load_catalog(library: &Library) -> Result<Vec<CatalogEntry>> {
    let path = catalog_path(library);
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text)?)
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
    fs::rename(tmp, path)?;
    Ok(())
}
