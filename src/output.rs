use cli_command_kit::{Mode, display_message};
use std::io::{self, Write};

fn terminal_text(message: &str) -> String {
    display_message(message)
        .replace(['\n', '\r', '\t'], " ")
        .chars()
        .filter(|c| !c.is_control())
        .collect()
}

pub fn status(out: &mut dyn Write, mode: Mode, kind: &str, message: &str) -> io::Result<()> {
    writeln!(
        out,
        "{} {}",
        if mode == Mode::Agent {
            format!("{kind}:")
        } else {
            format!("[{kind}]")
        },
        terminal_text(message)
    )
}
pub fn rows(
    out: &mut dyn Write,
    mode: Mode,
    headers: &[&str],
    rows: Vec<Vec<String>>,
    empty: &str,
) -> io::Result<()> {
    if rows.is_empty() {
        return writeln!(out, "{empty}");
    }
    let rows: Vec<Vec<String>> = rows
        .into_iter()
        .map(|row| row.into_iter().map(|v| terminal_text(&v)).collect())
        .collect();
    if mode == Mode::Agent {
        for row in rows {
            writeln!(
                out,
                "{}",
                headers
                    .iter()
                    .zip(row)
                    .map(|(key, value)| format!("{key}:{value}"))
                    .collect::<Vec<_>>()
                    .join("\t")
            )?;
        }
    } else {
        let mut table = comfy_table::Table::new();
        let width = cli_command_kit::terminal_width();
        table
            .load_preset(comfy_table::presets::ASCII_FULL)
            .set_content_arrangement(comfy_table::ContentArrangement::Dynamic)
            .set_width(if width == 0 {
                100
            } else {
                width.clamp(40, 160) as u16
            })
            .set_header(headers);
        for row in rows {
            table.add_row(row);
        }
        writeln!(out, "{table}")?;
    }
    Ok(())
}
pub fn json(out: &mut dyn Write, value: &impl serde::Serialize) -> Result<(), crate::Error> {
    serde_json::to_writer_pretty(&mut *out, value)?;
    writeln!(out)?;
    Ok(())
}
