//! Nests ("nidos") and acorns ("bellotas") in SQLite with FTS5 search.
//! Shared by the app (read/write) and `donguri-mcp` (read-only).

use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::Serialize;
use std::path::Path;

pub type Result<T> = rusqlite::Result<T>;

/// The nest that always exists and receives anything saved without a choice.
pub const UNSORTED: i64 = 1;

#[derive(Serialize, Clone, Debug)]
pub struct Nest {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub count: i64,
    pub chars: i64,
    pub system: bool,
    pub last_used: i64,
}

#[derive(Serialize, Clone, Debug)]
pub struct AcornSummary {
    pub id: i64,
    pub nest_id: i64,
    pub kind: String,
    pub title: String,
    pub preview: String,
    pub chars: i64,
    pub created_at: i64,
}

#[derive(Serialize, Clone, Debug)]
pub struct Acorn {
    pub id: i64,
    pub nest_id: i64,
    pub kind: String,
    pub title: String,
    pub content: String,
    pub file: Option<String>,
    pub source: Option<String>,
    pub created_at: i64,
}

#[derive(Serialize, Clone, Debug)]
pub struct Hit {
    pub acorn: AcornSummary,
    pub nest: String,
    pub snippet: String,
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

const SCHEMA: &str = "
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS nests (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE,
    color TEXT NOT NULL DEFAULT '#D97757',
    system INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    last_used INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS acorns (
    id INTEGER PRIMARY KEY,
    nest_id INTEGER NOT NULL REFERENCES nests(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    content TEXT NOT NULL DEFAULT '',
    file TEXT,
    source TEXT,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS acorns_nest ON acorns(nest_id, created_at);
CREATE VIRTUAL TABLE IF NOT EXISTS acorns_fts USING fts5(
    title, content, content='acorns', content_rowid='id', tokenize='unicode61 remove_diacritics 2'
);
CREATE TRIGGER IF NOT EXISTS acorns_ai AFTER INSERT ON acorns BEGIN
    INSERT INTO acorns_fts(rowid, title, content) VALUES (new.id, new.title, new.content);
END;
CREATE TRIGGER IF NOT EXISTS acorns_ad AFTER DELETE ON acorns BEGIN
    INSERT INTO acorns_fts(acorns_fts, rowid, title, content) VALUES ('delete', old.id, old.title, old.content);
END;
CREATE TRIGGER IF NOT EXISTS acorns_au AFTER UPDATE OF title, content ON acorns BEGIN
    INSERT INTO acorns_fts(acorns_fts, rowid, title, content) VALUES ('delete', old.id, old.title, old.content);
    INSERT INTO acorns_fts(rowid, title, content) VALUES (new.id, new.title, new.content);
END;
";

pub fn open(path: &Path) -> Result<Connection> {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let conn = Connection::open(path)?;
    conn.execute_batch(SCHEMA)?;
    conn.execute(
        "INSERT OR IGNORE INTO nests (id, name, system, color, created_at, last_used)
         VALUES (?1, 'Sin clasificar', 1, '#A97A35', ?2, 0)",
        params![UNSORTED, now()],
    )?;
    Ok(conn)
}

pub fn open_readonly(path: &Path) -> Result<Connection> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
}

// ------------------------------------------------------------------- nests

pub fn list_nests(c: &Connection) -> Result<Vec<Nest>> {
    let mut stmt = c.prepare(
        "SELECT n.id, n.name, n.color, n.system, n.last_used,
                COUNT(a.id), COALESCE(SUM(LENGTH(a.content)), 0)
         FROM nests n LEFT JOIN acorns a ON a.nest_id = n.id
         GROUP BY n.id ORDER BY n.system DESC, n.last_used DESC, n.name",
    )?;
    stmt.query_map([], |r| {
        Ok(Nest {
            id: r.get(0)?,
            name: r.get(1)?,
            color: r.get(2)?,
            system: r.get::<_, i64>(3)? != 0,
            last_used: r.get(4)?,
            count: r.get(5)?,
            chars: r.get(6)?,
        })
    })?
    .collect()
}

pub fn nest(c: &Connection, id: i64) -> Result<Option<Nest>> {
    Ok(list_nests(c)?.into_iter().find(|n| n.id == id))
}

pub fn nest_by_name(c: &Connection, name: &str) -> Result<Option<Nest>> {
    let name = name.trim();
    Ok(list_nests(c)?.into_iter().find(|n| n.name.eq_ignore_ascii_case(name) || n.name.to_lowercase() == name.to_lowercase()))
}

/// Creates the nest, or returns the existing one with that name.
pub fn create_nest(c: &Connection, name: &str, color: Option<&str>) -> Result<i64> {
    let name = name.trim();
    if let Some(existing) = nest_by_name(c, name)? {
        return Ok(existing.id);
    }
    c.execute(
        "INSERT INTO nests (name, color, created_at, last_used) VALUES (?1, ?2, ?3, ?3)",
        params![name, color.unwrap_or("#D97757"), now()],
    )?;
    Ok(c.last_insert_rowid())
}

pub fn rename_nest(c: &Connection, id: i64, name: &str) -> Result<()> {
    c.execute("UPDATE nests SET name = ?2 WHERE id = ?1 AND system = 0", params![id, name.trim()])?;
    Ok(())
}

pub fn set_nest_color(c: &Connection, id: i64, color: &str) -> Result<()> {
    c.execute("UPDATE nests SET color = ?2 WHERE id = ?1", params![id, color])?;
    Ok(())
}

/// Deletes a nest and its acorns; returns the files those acorns owned.
pub fn delete_nest(c: &Connection, id: i64) -> Result<Vec<String>> {
    if id == UNSORTED {
        return Ok(Vec::new());
    }
    let files = c
        .prepare("SELECT file FROM acorns WHERE nest_id = ?1 AND file IS NOT NULL")?
        .query_map([id], |r| r.get(0))?
        .collect::<Result<Vec<String>>>()?;
    c.execute("DELETE FROM nests WHERE id = ?1", [id])?;
    Ok(files)
}

pub fn touch_nest(c: &Connection, id: i64) -> Result<()> {
    c.execute("UPDATE nests SET last_used = ?2 WHERE id = ?1", params![id, now()])?;
    Ok(())
}

// ------------------------------------------------------------------ acorns

pub struct NewAcorn<'a> {
    pub nest_id: i64,
    pub kind: &'a str,
    pub title: &'a str,
    pub content: &'a str,
    pub file: Option<&'a str>,
    pub source: Option<&'a str>,
}

pub fn add_acorn(c: &Connection, a: NewAcorn) -> Result<i64> {
    c.execute(
        "INSERT INTO acorns (nest_id, kind, title, content, file, source, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![a.nest_id, a.kind, a.title, a.content, a.file, a.source, now()],
    )?;
    let id = c.last_insert_rowid();
    touch_nest(c, a.nest_id)?;
    Ok(id)
}

fn preview(text: &str) -> String {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut p: String = flat.chars().take(140).collect();
    if flat.chars().count() > 140 {
        p.push('…');
    }
    p
}

fn summary(r: &rusqlite::Row) -> Result<AcornSummary> {
    let content: String = r.get(4)?;
    Ok(AcornSummary {
        id: r.get(0)?,
        nest_id: r.get(1)?,
        kind: r.get(2)?,
        title: r.get(3)?,
        chars: content.chars().count() as i64,
        preview: preview(&content),
        created_at: r.get(5)?,
    })
}

pub fn list_acorns(c: &Connection, nest_id: i64) -> Result<Vec<AcornSummary>> {
    c.prepare(
        "SELECT id, nest_id, kind, title, content, created_at FROM acorns
         WHERE nest_id = ?1 ORDER BY created_at DESC, id DESC",
    )?
    .query_map([nest_id], summary)?
    .collect()
}

/// File names of the newest image acorns of a nest.
pub fn nest_images(c: &Connection, nest_id: i64, limit: usize) -> Result<Vec<String>> {
    c.prepare(
        "SELECT file FROM acorns WHERE nest_id = ?1 AND kind = 'imagen' AND file IS NOT NULL
         ORDER BY created_at DESC, id DESC LIMIT ?2",
    )?
    .query_map(rusqlite::params![nest_id, limit as i64], |r| r.get(0))?
    .collect()
}

pub fn acorn(c: &Connection, id: i64) -> Result<Option<Acorn>> {
    c.query_row(
        "SELECT id, nest_id, kind, title, content, file, source, created_at FROM acorns WHERE id = ?1",
        [id],
        |r| {
            Ok(Acorn {
                id: r.get(0)?,
                nest_id: r.get(1)?,
                kind: r.get(2)?,
                title: r.get(3)?,
                content: r.get(4)?,
                file: r.get(5)?,
                source: r.get(6)?,
                created_at: r.get(7)?,
            })
        },
    )
    .optional()
}

pub fn move_acorn(c: &Connection, id: i64, nest_id: i64) -> Result<()> {
    c.execute("UPDATE acorns SET nest_id = ?2 WHERE id = ?1", params![id, nest_id])?;
    touch_nest(c, nest_id)
}

/// Rewrites a text acorn (title and content), e.g. edited by hand in Kura.
pub fn edit_acorn(c: &Connection, id: i64, title: &str, content: &str) -> Result<()> {
    c.execute("UPDATE acorns SET title = ?2, content = ?3 WHERE id = ?1", params![id, title.trim(), content])?;
    Ok(())
}

/// Deletes an acorn; returns the file it owned, if any.
pub fn delete_acorn(c: &Connection, id: i64) -> Result<Option<String>> {
    let file = acorn(c, id)?.and_then(|a| a.file);
    c.execute("DELETE FROM acorns WHERE id = ?1", [id])?;
    Ok(file)
}

// ------------------------------------------------------------------ search

/// Turns free text into a safe FTS5 query: every word quoted, prefix-matched.
fn fts_query(q: &str) -> Option<String> {
    let terms: Vec<String> = q
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|t| t.chars().count() > 1)
        .map(|t| format!("\"{}\"*", t.replace('"', "")))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" OR "))
}

pub fn search(c: &Connection, query: &str, nest_id: Option<i64>, limit: usize) -> Result<Vec<Hit>> {
    let Some(q) = fts_query(query) else { return Ok(Vec::new()) };
    let mut stmt = c.prepare(
        "SELECT a.id, a.nest_id, a.kind, a.title, a.content, a.created_at, n.name,
                snippet(acorns_fts, 1, '«', '»', '…', 16)
         FROM acorns_fts f JOIN acorns a ON a.id = f.rowid JOIN nests n ON n.id = a.nest_id
         WHERE acorns_fts MATCH ?1 AND (?2 IS NULL OR a.nest_id = ?2)
         ORDER BY bm25(acorns_fts, 4.0, 1.0) LIMIT ?3",
    )?;
    stmt.query_map(params![q, nest_id, limit as i64], |r| {
        Ok(Hit { acorn: summary(r)?, nest: r.get(6)?, snippet: r.get(7)? })
    })?
    .collect()
}

// ---------------------------------------------------------------- context

pub struct NestText {
    pub text: String,
    /// The nest did not fit; only the most relevant acorns were included.
    pub truncated: bool,
    pub total_chars: usize,
}

fn render_acorn(a: &Acorn) -> String {
    let content = a.content.trim();
    if content.is_empty() {
        // No text (an image): say what it is and its id, so an AI can ask for it.
        return format!("### {}\n\n[{} · bellota {}]\n", a.title, a.kind, a.id);
    }
    // Titles made from the first line add nothing: skip the heading then.
    let stem = a.title.trim_end_matches('…').trim();
    if !stem.is_empty() && content.starts_with(stem) {
        return format!("---\n\n{content}\n");
    }
    format!("### {}\n\n{content}\n", a.title)
}

/// The nest as Markdown, fitted into `budget` characters. When it does not
/// fit, acorns matching `query` go first, then the newest ones.
pub fn nest_text(c: &Connection, nest_id: i64, budget: usize, query: Option<&str>) -> Result<NestText> {
    let ids: Vec<i64> = list_acorns(c, nest_id)?.into_iter().map(|a| a.id).collect();
    let acorns: Vec<Acorn> = ids.iter().filter_map(|id| acorn(c, *id).ok().flatten()).collect();
    let rendered: Vec<String> = acorns.iter().map(render_acorn).collect();
    let total: usize = rendered.iter().map(|r| r.chars().count()).sum();
    if total <= budget {
        // Oldest first reads more naturally as a document.
        let text = rendered.iter().rev().cloned().collect::<Vec<_>>().join("\n");
        return Ok(NestText { text, truncated: false, total_chars: total });
    }

    let mut order: Vec<usize> = Vec::new();
    if let Some(q) = query {
        for hit in search(c, q, Some(nest_id), 200)? {
            if let Some(i) = acorns.iter().position(|a| a.id == hit.acorn.id) {
                order.push(i);
            }
        }
    }
    let rest: Vec<usize> = (0..acorns.len()).filter(|i| !order.contains(i)).collect();
    order.extend(rest);

    let mut used = 0;
    let mut parts = Vec::new();
    for i in order {
        let len = rendered[i].chars().count();
        if used + len > budget {
            continue;
        }
        used += len;
        parts.push(rendered[i].clone());
    }
    Ok(NestText { text: parts.join("\n"), truncated: true, total_chars: total })
}

pub fn export_markdown(c: &Connection, nest_id: i64) -> Result<String> {
    let name = nest(c, nest_id)?.map(|n| n.name).unwrap_or_default();
    let body = nest_text(c, nest_id, usize::MAX, None)?.text;
    Ok(format!("# {name}\n\n{body}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        // One file per test: tests run in parallel and a clock can repeat.
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("donguri-test-{}", std::process::id()));
        let path = dir.join(format!("{n}-{}.db", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)));
        open(&path).unwrap()
    }

    fn add(c: &Connection, nest: i64, title: &str, content: &str) -> i64 {
        add_acorn(c, NewAcorn { nest_id: nest, kind: "texto", title, content, file: None, source: None }).unwrap()
    }

    #[test]
    fn unsorted_always_exists_and_cannot_be_deleted() {
        let c = db();
        assert!(nest(&c, UNSORTED).unwrap().unwrap().system);
        delete_nest(&c, UNSORTED).unwrap();
        assert!(nest(&c, UNSORTED).unwrap().is_some());
    }

    #[test]
    fn create_nest_is_case_insensitive() {
        let c = db();
        let a = create_nest(&c, "Viaje", None).unwrap();
        let b = create_nest(&c, "viaje", None).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn search_finds_prefixes_and_ignores_accents() {
        let c = db();
        let n = create_nest(&c, "Viaje", None).unwrap();
        add(&c, n, "Kioto", "Excursión de un día a Kioto con trenes");
        assert_eq!(search(&c, "tren", None, 10).unwrap().len(), 1);
        assert_eq!(search(&c, "excursion", Some(n), 10).unwrap().len(), 1);
        assert!(search(&c, "\"; DROP TABLE", None, 10).unwrap().is_empty());
    }

    #[test]
    fn nest_text_trims_to_budget_and_prefers_matches() {
        let c = db();
        let n = create_nest(&c, "Big", None).unwrap();
        add(&c, n, "Uno", &"relleno ".repeat(200));
        add(&c, n, "Clave", "la respuesta está en la sección del Shinkansen");
        add(&c, n, "Tres", &"más relleno ".repeat(200));
        let full = nest_text(&c, n, usize::MAX, None).unwrap();
        assert!(!full.truncated);
        let small = nest_text(&c, n, 200, Some("Shinkansen")).unwrap();
        assert!(small.truncated);
        assert!(small.text.contains("Shinkansen"));
        assert!(small.text.chars().count() <= 200);
    }

    #[test]
    fn edited_acorns_are_found_by_their_new_words() {
        let c = db();
        let a = add(&c, UNSORTED, "Nota", "primer borrador");
        edit_acorn(&c, a, "Nota", "texto corregido").unwrap();
        assert!(search(&c, "borrador", None, 10).unwrap().is_empty());
        assert_eq!(search(&c, "corregido", None, 10).unwrap().len(), 1);
    }

    #[test]
    fn deleting_a_nest_removes_its_acorns() {
        let c = db();
        let n = create_nest(&c, "Tmp", None).unwrap();
        let a = add(&c, n, "x", "y");
        delete_nest(&c, n).unwrap();
        assert!(acorn(&c, a).unwrap().is_none());
    }
}

/// Words in a text. Chinese, Japanese and Korean are written without spaces,
/// so each of their characters counts as a word there.
pub fn word_count(text: &str) -> usize {
    let cjk = |c: char| matches!(c as u32, 0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF);
    text.split_whitespace()
        .map(|w| {
            let n = w.chars().filter(|&c| cjk(c)).count();
            if n == 0 { 1 } else { n + usize::from(w.chars().any(|c| c.is_alphanumeric() && !cjk(c))) }
        })
        .sum()
}

#[cfg(test)]
mod word_tests {
    use super::word_count;

    #[test]
    fn counts_spaced_and_cjk_words() {
        assert_eq!(word_count("hola mundo  bonito"), 3);
        assert_eq!(word_count("BBC驻北美记者"), 6);
        assert_eq!(word_count(""), 0);
    }
}
