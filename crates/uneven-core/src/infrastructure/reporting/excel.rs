use crate::UnevenError;
use rust_xlsxwriter::*;
use serde_json::Value;
use std::path::Path;

pub struct ExcelReport {
    workbook: Workbook,
}

impl ExcelReport {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for ExcelReport {
    fn default() -> Self {
        let mut workbook = Workbook::new();
        workbook.set_properties(&DocProperties::new()
            .set_author("Uneven Data Analyst")
            .set_title("Analysis Report"));
        Self { workbook }
    }
}

impl ExcelReport {

    pub fn add_sheet(&mut self, name: &str, columns: &[String], rows: &[Value]) -> Result<(), UnevenError> {
        let worksheet = self.workbook.add_worksheet();
        worksheet.set_name(name).map_err(|e| UnevenError::KnowledgeError(e.to_string()))?;

        // Formatting
        let header_format = Format::new()
            .set_bold()
            .set_font_color(Color::White)
            .set_background_color(Color::RGB(0x1F4E79))
            .set_align(FormatAlign::Center);

        let zebra_format = Format::new()
            .set_background_color(Color::RGB(0xF0F4F8));

        // Headers
        for (i, col) in columns.iter().enumerate() {
            worksheet.write_with_format(0, i as u16, col, &header_format)
                .map_err(|e| UnevenError::KnowledgeError(e.to_string()))?;
        }

        // Data
        for (row_idx, row) in rows.iter().enumerate() {
            let row_num = (row_idx + 1) as u32;
            let current_format = if row_num.is_multiple_of(2) { Some(&zebra_format) } else { None };

            if let Value::Object(map) = row {
                for (col_idx, col_name) in columns.iter().enumerate() {
                    let val = map.get(col_name).unwrap_or(&Value::Null);
                    let col_num = col_idx as u16;
                    
                    match val {
                        Value::String(s) => {
                            if let Some(f) = current_format {
                                worksheet.write_with_format(row_num, col_num, s, f).ok();
                            } else {
                                worksheet.write(row_num, col_num, s).ok();
                            }
                        }
                        Value::Number(n) => {
                            if let Some(f) = current_format {
                                worksheet.write_with_format(row_num, col_num, n.as_f64().unwrap_or(0.0), f).ok();
                            } else {
                                worksheet.write(row_num, col_num, n.as_f64().unwrap_or(0.0)).ok();
                            }
                        }
                        Value::Bool(b) => {
                            if let Some(f) = current_format {
                                worksheet.write_with_format(row_num, col_num, *b, f).ok();
                            } else {
                                worksheet.write(row_num, col_num, *b).ok();
                            }
                        }
                        _ => {
                            if let Some(f) = current_format {
                                worksheet.write_with_format(row_num, col_num, "", f).ok();
                            } else {
                                worksheet.write(row_num, col_num, "").ok();
                            }
                        }
                    }
                }
            }
        }

        // Auto-fit (Simplified)
        for i in 0..columns.len() as u16 {
            worksheet.set_column_width(i, 20).ok();
        }

        Ok(())
    }

    pub fn save<P: AsRef<Path>>(&mut self, path: P) -> Result<(), UnevenError> {
        self.workbook.save(path).map_err(|e| UnevenError::KnowledgeError(e.to_string()))
    }
}
