pub fn build_analysis_prompt(schema_text: &str) -> String {
    format!(
        "You are a data analyst. Based on the following database schema, suggest exactly 5 concise analysis questions a business user would find valuable.\n\n\
         Schema:\n\
         {}\n\n\
         STRICT OUTPUT FORMAT:\n\
         Respond ONLY with the suggestions wrapped in the following tag. Each suggestion on its own line, starting with \"•\". No conversational filler.\n\n\
         <suggestions>\n\
         • Suggestion 1\n\
         • Suggestion 2\n\
         ...\n\
         </suggestions>",
        schema_text
    ).trim().to_string()
}

pub fn build_sql_gen_prompt(
    db_type: Option<&str>,
    schema_text: &str,
    user_request: &str,
    prompt_rules: &str,
) -> String {
    format!(
        "You are an expert Data & BI Analyst. Generate the analytical SELECT queries and dashboard specification for the following request.\n\n\
         Database type: {}\n\n\
         Schema (sensitive columns have already been redacted/removed):\n\
         {}\n\n\
         User request: \"{}\"\n\n\
         {}\n\n\
         Additional rules:\n\
         - ONLY generate SELECT statements. Never INSERT, UPDATE, DELETE, DROP, ALTER, TRUNCATE or any DDL/DML.\n\
         - MULTI-TABLE / CROSS-DATASET SUPPORT: If the user request requires examining multiple tables with different grains (e.g. sales aggregated by territory AND top returned products), you can provide multiple named SQL blocks: ```sql:dataset_name ... ```. Use concise dataset names (e.g. sales_summary, budget_breakdown).\n\
         - If you cannot generate a valid query for this schema, respond with exactly: CANNOT_GENERATE followed by a one-sentence explanation.\n\
         - If the request violates any SECURITY RULE, respond with exactly: SECURITY_BLOCKED\n\n\
         STRICT OUTPUT FORMAT (MARKDOWN):\n\
         Output your response strictly using Markdown. Place the SQL query inside ```sql code blocks and the dashboard spec inside a ```dashboard code block.\n\n\
         ### Explanation\n\
         A very brief technical summary of the query.\n\n\
         ### Report\n\
         A high-level executive report of what this query will uncover.\n\n\
         ```sql:dataset_main\n\
         SELECT ...\n\
         ```\n\n\
         ```dashboard\n\
         title: \"Executive Analysis Report\"\n\
         theme: dark\n\
         palette: [\"#10b981\", \"#06b6d4\", \"#8b5cf6\"]\n\
         layout:\n\
           - type: kpi\n\
             dataset: dataset_main\n\
             metric: total_amount\n\
             label: \"Total Value\"\n\
             format: currency\n\
             span: 4\n\
           - type: chart\n\
             dataset: dataset_main\n\
             chartType: bar\n\
             title: \"Performance Ranking\"\n\
             x: category\n\
             y: total_amount\n\
             span: 8\n\
           - type: table\n\
             dataset: dataset_main\n\
             title: \"Detailed Records\"\n\
             span: 12\n\
         ```",
        db_type.unwrap_or("SQL"),
        schema_text,
        user_request,
        prompt_rules
    ).trim().to_string()
}

pub fn build_style_extraction_prompt(user_style_prompt: &str, columns: &[String]) -> String {
    format!(
        "You are the Lead Architect for a BI Dashboard. Translate the following user style request into dashboard configuration.\n\n\
         User request: \"{}\"\n\n\
         Available Data Columns from the executed query: {}\n\n\
         STRICT OUTPUT FORMAT:\n\
         You MUST format your response as follows. Do NOT write any conversational text.\n\n\
         VISUAL:\n\
         title=<Report Title> | theme=<dark|light> | layout=[{{ \"type\": \"card\", \"config\": {{ \"title\": \"Total Users\", \"metricCol\": \"count\", \"icon\": \"users\", \"colSpan\": 3 }} }}, {{ \"type\": \"chart\", \"config\": {{ \"chartType\": \"bar\", \"title\": \"Salaries by Dept\", \"labelCol\": \"dept_name\", \"valueCols\": [\"total_salary\"], \"colSpan\": 9, \"height\": 300 }} }}] | colors=<#primary,#accent,#bg,#text>\n\n\
         - ALWAYS use the exact column names from the Available Data Columns list in \"metricCol\", \"labelCol\", and \"valueCols\".\n\
         - COLOR RULE: Translate user style requests into a list of hex codes. The order MUST be: [primary, accent, background, text]. If the user doesn't specify background/text, provide only [primary, accent]. These colors define the GLOBAL theme via CSS variables.\n\
         - CHART RULE: Prioritize user-requested components (e.g. \"2 cards, 1 bars\").\n\
         - Standard Executive Window: 12 columns width. Use \"colSpan\" (1-12) to balance the grid.\n\
         - Bento Grid aesthetic: Mix small cards (colSpan 3, height 120), charts (colSpan 6-9, height 300), and tables (type: \"table\", colSpan 12).",
        user_style_prompt,
        columns.join(", ")
    ).trim().to_string()
}
