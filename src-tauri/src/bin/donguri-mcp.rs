//! `donguri-mcp`: a local, read-only MCP server over the Donguri nests.
//!
//! Usage: donguri-mcp --db <path to donguri.db>
//! Speaks MCP over stdio; it never writes to the database.

use donguri_lib::store;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    schemars, tool, tool_handler, tool_router, transport::stdio, ServerHandler, ServiceExt,
};
use serde::Deserialize;
use std::path::PathBuf;

/// Rough characters-per-token ratio used to honour `max_tokens`.
const CHARS_PER_TOKEN: usize = 4;
const DEFAULT_MAX_TOKENS: usize = 50_000;

#[derive(Clone)]
struct Donguri {
    db: PathBuf,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct SearchArgs {
    /// Words to look for in the saved acorns.
    consulta: String,
    /// Optional nest name to search only inside it.
    nido: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct ReadNestArgs {
    /// Nest name, as shown by listar_nidos.
    nombre: String,
    /// Optional token budget; if the nest is bigger, the most recent acorns are returned.
    max_tokens: Option<usize>,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct ReadAcornArgs {
    /// Acorn id, as shown by buscar.
    id: i64,
}

impl Donguri {
    /// Saved files (images, PDFs) live next to the database.
    fn files(&self) -> PathBuf {
        self.db.parent().map(|d| d.join("bellotas")).unwrap_or_default()
    }

    fn conn(&self) -> Result<rusqlite::Connection, String> {
        store::open_readonly(&self.db).map_err(|e| format!("No puedo abrir los nidos de Donguri: {e}"))
    }
}

#[tool_router]
impl Donguri {
    #[tool(description = "Lista los nidos (temas) de Donguri con cuántas bellotas (notas guardadas) tiene cada uno.")]
    fn listar_nidos(&self) -> Result<String, String> {
        let c = self.conn()?;
        let nests = store::list_nests(&c).map_err(|e| e.to_string())?;
        if nests.iter().all(|n| n.count == 0) {
            return Ok("El usuario aún no ha guardado nada en Donguri.".into());
        }
        Ok(nests
            .iter()
            .filter(|n| n.count > 0)
            .map(|n| format!("- {} ({} bellotas, ~{} tokens)", n.name, n.count, n.chars as usize / CHARS_PER_TOKEN))
            .collect::<Vec<_>>()
            .join("\n"))
    }

    #[tool(description = "Busca texto en las bellotas guardadas en Donguri, opcionalmente dentro de un nido. Devuelve ids para leer_bellota.")]
    fn buscar(&self, Parameters(args): Parameters<SearchArgs>) -> Result<String, String> {
        let c = self.conn()?;
        let nest_id = match &args.nido {
            Some(name) => Some(
                store::nest_by_name(&c, name)
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| format!("No existe el nido «{name}»."))?
                    .id,
            ),
            None => None,
        };
        let hits = store::search(&c, &args.consulta, nest_id, 20).map_err(|e| e.to_string())?;
        if hits.is_empty() {
            return Ok(format!("Nada encontrado para «{}».", args.consulta));
        }
        Ok(hits
            .iter()
            .map(|h| format!("- [{}] {} (nido: {}): {}", h.acorn.id, h.acorn.title, h.nest, h.snippet))
            .collect::<Vec<_>>()
            .join("\n"))
    }

    #[tool(description = "Lee el contenido completo de un nido de Donguri como Markdown. Úsalo cuando el usuario diga «usa mi nido X».")]
    fn leer_nido(&self, Parameters(args): Parameters<ReadNestArgs>) -> Result<String, String> {
        let c = self.conn()?;
        let nest = store::nest_by_name(&c, &args.nombre)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("No existe el nido «{}». Usa listar_nidos.", args.nombre))?;
        let budget = args.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS).saturating_mul(CHARS_PER_TOKEN);
        let t = store::nest_text(&c, nest.id, budget, None).map_err(|e| e.to_string())?;
        if t.text.is_empty() {
            return Ok(format!("El nido «{}» está vacío.", nest.name));
        }
        let note = if t.truncated {
            format!("\n\n(Nido recortado: se muestran {} de ~{} caracteres. Usa buscar para lo demás.)", t.text.chars().count(), t.total_chars)
        } else {
            String::new()
        };
        Ok(format!("# Nido: {}\n\n{}{}", nest.name, t.text, note))
    }

    #[tool(description = "Lee una bellota concreta de Donguri por su id. Si es una imagen (una captura), devuelve la imagen.")]
    fn leer_bellota(&self, Parameters(args): Parameters<ReadAcornArgs>) -> Result<CallToolResult, String> {
        let c = self.conn()?;
        let a = store::acorn(&c, args.id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("No existe la bellota {}.", args.id))?;
        let heading = format!("### {}", a.title);
        if a.kind == "imagen"
            && let Some(png) = a.file.as_deref().and_then(|f| std::fs::read(self.files().join(f)).ok())
        {
            use base64::Engine;
            let data = base64::engine::general_purpose::STANDARD.encode(png);
            return Ok(CallToolResult::success(vec![ContentBlock::text(heading), ContentBlock::image(data, "image/png")]));
        }
        let body = if a.content.trim().is_empty() { format!("[{} sin texto]", a.kind) } else { a.content };
        Ok(CallToolResult::success(vec![ContentBlock::text(format!("{heading}\n\n{body}"))]))
    }
}

#[tool_handler(
    name = "donguri",
    instructions = "Donguri guarda notas del usuario («bellotas») agrupadas en «nidos» por tema. Cuando el usuario diga «usa mi nido X» o mencione un nido, llama a leer_nido con ese nombre y usa su contenido como contexto. Usa listar_nidos si no sabes qué nidos hay y buscar para encontrar algo concreto. Las imágenes aparecen como «[imagen · bellota N]»: léelas con leer_bellota."
)]
impl ServerHandler for Donguri {}

fn db_path() -> Option<PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--db" {
            return args.next().map(PathBuf::from);
        }
    }
    None
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = db_path().ok_or("usage: donguri-mcp --db <path>")?;
    let service = Donguri { db }.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
