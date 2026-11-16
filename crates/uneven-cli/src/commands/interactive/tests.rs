use super::*;
use uneven_core::domain::actions::extract_action_response as extract_json;
use uneven_core::domain::config::UnevenConfig;

    #[test]
    fn test_classify_intent_utility() {
        assert_eq!(classify_intent("exit").action, ShellAction::Exit);
        assert_eq!(classify_intent("help").action, ShellAction::Help);
        assert_eq!(classify_intent("log").action, ShellAction::Log);
        assert_eq!(classify_intent("undo").action, ShellAction::Undo);
        assert_eq!(classify_intent("index").action, ShellAction::Index);
        assert_eq!(classify_intent("scan").action, ShellAction::Scan);
        assert_eq!(classify_intent("watch").action, ShellAction::Watch);
    }

    #[test]
    fn test_classify_intent_explain_and_docs() {
        let explain_intent = classify_intent("explain src/app.ts");
        assert_eq!(explain_intent.action, ShellAction::Explain);
        assert_eq!(explain_intent.file, Some("src/app.ts".to_string()));

        let explain_bare = classify_intent("src/app.ts");
        assert_eq!(explain_bare.action, ShellAction::Explain);
        assert_eq!(explain_bare.file, Some("src/app.ts".to_string()));

        let docs_intent = classify_intent("docs src/utils.js");
        assert_eq!(docs_intent.action, ShellAction::Docs);
        assert_eq!(docs_intent.file, Some("src/utils.js".to_string()));
    }

    #[test]
    fn test_classify_intent_review() {
        let review1 = classify_intent("review --commit abc1234");
        assert_eq!(review1.action, ShellAction::Think);
        assert_eq!(review1.task, Some("--commit abc1234".to_string()));

        let review2 = classify_intent("review");
        assert_eq!(review2.action, ShellAction::Think);
        assert_eq!(review2.task, Some("review changes".to_string()));
    }

    #[test]
    fn test_classify_intent_fallback() {
        // Natural language query without prefix -> Think (reasoning, investigation, and code proposals)
        let ask_intent = classify_intent("how to configure the database?");
        assert_eq!(ask_intent.action, ShellAction::Think);
        assert_eq!(ask_intent.task, Some("how to configure the database?".to_string()));

        let ask_ola = classify_intent("hello");
        assert_eq!(ask_ola.action, ShellAction::Think);

        let ask_natural = classify_intent("create an authentication route with jwt");
        assert_eq!(ask_natural.action, ShellAction::Think);

        // With askf prefix -> Askf
        let askf_intent = classify_intent("askf create an authentication route with jwt");
        assert_eq!(askf_intent.action, ShellAction::Askf);
        assert_eq!(askf_intent.task, Some("create an authentication route with jwt".to_string()));
    }

    #[test]
    fn test_tokenize_shell_input() {
        let tokens = tokenize_shell_input("scan -d \"./src dir\" --report");
        assert_eq!(tokens, vec!["scan", "-d", "./src dir", "--report"]);

        let tokens_single = tokenize_shell_input("explain 'src/utils/math.rs'");
        assert_eq!(tokens_single, vec!["explain", "src/utils/math.rs"]);

        let escaped = tokenize_shell_input("hello\\ world");
        assert_eq!(escaped, vec!["hello world"]);
    }

    #[test]
    fn test_parse_analyze_input() {
        // Pure prompt without flags
        let parsed = parse_analyze_input("how is the revenue?");
        assert_eq!(
            parsed,
            vec![
                "analyze".to_string(),
                "--prompt".to_string(),
                "how is the revenue?".to_string()
            ]
        );

        // Mixed flags and prompt
        let parsed_mixed = parse_analyze_input("which table has more records --theme light --report html");
        assert_eq!(
            parsed_mixed,
            vec![
                "analyze".to_string(),
                "--theme".to_string(),
                "light".to_string(),
                "--report".to_string(),
                "html".to_string(),
                "--prompt".to_string(),
                "which table has more records".to_string()
            ]
        );

        // Flags only
        let parsed_only_flags = parse_analyze_input("--clear-cache");
        assert_eq!(parsed_only_flags, vec!["analyze".to_string(), "--clear-cache".to_string()]);
    }

    #[test]
    fn test_parse_all_need_file_tags() {
        let text = r#"
            Hello, please check [NEED_FILE: src/main.rs]
            Also need_file:src/utils.rs and unclosed [NEED_FILE: src/app.ts
            Maybe json {"need_file": "src/theme.rs"} or single quote NEED_FILE:'src/config.rs'
            And trailing comma need_file:src/db.rs, thanks!
            Sentence ending with dot need_file:src/router.rs.
            Or questioning need_file:src/auth.rs?
            With equals sign NEED_FILE=src/services.ts and angle brackets NEED_FILE: <src/handler.go>
            With parenthesis NEED_FILE (src/models.py) and spaces NEED_FILE src/views.py
        "#;
        let tags = parse_all_need_file_tags(text);
        assert!(tags.contains("src/main.rs"));
        assert!(tags.contains("src/utils.rs"));
        assert!(tags.contains("src/app.ts"));
        assert!(tags.contains("src/theme.rs"));
        assert!(tags.contains("src/config.rs"));
        assert!(tags.contains("src/db.rs"));
        assert!(tags.contains("src/router.rs"));
        assert!(tags.contains("src/auth.rs"));
        assert!(tags.contains("src/services.ts"));
        assert!(tags.contains("src/handler.go"));
        assert!(tags.contains("src/models.py"));
        assert!(tags.contains("src/views.py"));
        assert_eq!(tags.len(), 12);
    }

    #[test]
    fn test_parse_all_web_search_and_url_tags() {
        let text = r#"
            Please search Google for [WEB_SEARCH: actix-web 4.5 routing examples]
            Also try WEB_SEARCH: "rust tokio tutorial" and WEB_SEARCH: 'uneven agent'
            And check the documentation at [WEB_URL: https://actix.rs/docs/routing/]
            Also WEB_URL: https://github.com/uneven-agent/uneven
        "#;
        let searches = parse_all_web_search_tags(text);
        assert!(searches.contains("actix-web 4.5 routing examples"));
        assert!(searches.contains("rust tokio tutorial"));
        assert!(searches.contains("uneven agent"));
        assert_eq!(searches.len(), 3);

        let urls = parse_all_web_url_tags(text);
        assert!(urls.contains("https://actix.rs/docs/routing/"));
        assert!(urls.contains("https://github.com/uneven-agent/uneven"));
        assert_eq!(urls.len(), 2);
    }

    #[test]
    fn test_parse_multi_brains_no_explicit() {
        let parsed = parse_multi_brains("gemini 3.5 flash opus 4.6 gpt 5.5").unwrap();
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0], (None, "gemini-3.5-flash".to_string(), "gemini".to_string()));
        assert_eq!(parsed[1], (None, "claude-opus-4".to_string(), "claude".to_string()));
        assert_eq!(parsed[2], (None, "gpt-5.5".to_string(), "openai".to_string()));
    }

    #[test]
    fn test_parse_multi_brains_explicit_with_typo() {
        let parsed = parse_multi_brains("explan gemini 3.5 flash pentest sonnet 3").unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0], (Some("explain".to_string()), "gemini-3.5-flash".to_string(), "gemini".to_string()));
        assert_eq!(parsed[1], (Some("pentest".to_string()), "claude-3-5-sonnet-latest".to_string(), "claude".to_string()));
    }

    #[test]
    fn test_parse_multi_brains_with_typo() {
        let parsed = parse_multi_brains("hemini 3.5 flash");
        assert!(parsed.is_err());
        let err = parsed.unwrap_err();
        assert!(err.contains("Unknown model provider in \"hemini 3.5 flash\". Did you mean \"gemini\"?"));
    }

    #[test]
    fn test_allocate_brains_smart_routing() {
        let mut config = UnevenConfig::default();
        let parsed = parse_multi_brains("gemini 3.5 flash opus 4.6 gpt 5.5").unwrap();
        allocate_brains(parsed, &mut config);

        let brains = config.brains.as_ref().unwrap();
        assert_eq!(brains.len(), 3);

        assert_eq!(config.get_brain_for_command("watch").id.unwrap(), "claude-claude-opus-4");
        assert_eq!(config.get_brain_for_command("explain").id.unwrap(), "gemini-gemini-3.5-flash");
        assert_eq!(config.get_brain_for_command("chat").id.unwrap(), "gemini-gemini-3.5-flash");
        assert_eq!(config.get_brain_for_command("analyze").id.unwrap(), "gemini-gemini-3.5-flash");
        assert_eq!(config.get_brain_for_command("scan").id.unwrap(), "openai-gpt-5.5");
        assert_eq!(config.get_brain_for_command("zeroday").id.unwrap(), "openai-gpt-5.5");

        // Assert dynamic temperature mappings
        assert_eq!(config.get_brain_for_command("chat").temperature, 0.7);
        assert_eq!(config.get_brain_for_command("explain").temperature, 0.2);
        assert_eq!(config.get_brain_for_command("watch").temperature, 0.2);
    }

    #[test]
    fn test_allocate_brains_local_models() {
        let mut config = UnevenConfig::default();
        let parsed = parse_multi_brains("llama qwen").unwrap();
        allocate_brains(parsed, &mut config);

        let brains = config.brains.as_ref().unwrap();
        assert_eq!(brains.len(), 2);
        assert_eq!(brains[0].provider, "local");
        assert_eq!(brains[0].model, "llama-3.2-1b-q4");
        assert!(brains[0].local.is_some());

        assert_eq!(brains[1].provider, "local");
        assert_eq!(brains[1].model, "qwen-2.5-1.5b-q8");
        assert!(brains[1].local.is_some());
    }

    #[test]
    fn test_slash_analyze_parsing_logic() {
        let mut config = UnevenConfig::default();

        // 1. db url
        let db_url = "postgres://localhost/test_db";
        let mut db_cfg = config.knowledge.as_ref().and_then(|k| k.db.clone()).unwrap_or_default();
        db_cfg.url = Some(db_url.to_string());
        let mut k_cfg = config.knowledge.clone().unwrap_or_default();
        k_cfg.db = Some(db_cfg);
        config.knowledge = Some(k_cfg);

        assert_eq!(config.knowledge.as_ref().unwrap().db.as_ref().unwrap().url.as_ref().unwrap(), "postgres://localhost/test_db");

        // 2. allowed tables
        let tables = vec!["users".to_string(), "orders".to_string()];
        let mut db_cfg = config.knowledge.as_ref().and_then(|k| k.db.clone()).unwrap_or_default();
        db_cfg.allowed_tables = Some(tables.clone());
        let mut k_cfg = config.knowledge.clone().unwrap_or_default();
        k_cfg.db = Some(db_cfg);
        config.knowledge = Some(k_cfg);

        assert_eq!(config.knowledge.as_ref().unwrap().db.as_ref().unwrap().allowed_tables.as_ref().unwrap(), &vec!["users".to_string(), "orders".to_string()]);

        // 3. mask columns and tables
        let m_tables = vec!["salary".to_string()];
        let m_columns = vec!["cpf".to_string(), "password".to_string()];
        config.security.blocked_tables = m_tables;
        config.security.masked_columns = m_columns;

        assert_eq!(config.security.blocked_tables, vec!["salary".to_string()]);
        assert_eq!(config.security.masked_columns, vec!["cpf".to_string(), "password".to_string()]);
    }

    #[test]
    fn test_slash_pentest_parsing_logic() {
        let mut config = UnevenConfig::default();

        let mut p_cfg = config.pentester.clone().unwrap_or_default();
        p_cfg.target = Some("http://localhost:3000".to_string());
        config.pentester = Some(p_cfg);

        assert_eq!(config.pentester.as_ref().unwrap().target.as_ref().unwrap(), "http://localhost:3000");

        let mut p_cfg = config.pentester.clone().unwrap_or_default();
        p_cfg.mode = Some(uneven_core::domain::PentesterMode::Active);

        let mut s_cfg = p_cfg.static_config.clone().unwrap_or_default();
        s_cfg.owasp = Some(true);
        s_cfg.secrets = Some(false);
        p_cfg.static_config = Some(s_cfg);

        let mut b_cfg = p_cfg.bruteforce.clone().unwrap_or_default();
        b_cfg.enabled = Some(true);
        p_cfg.bruteforce = Some(b_cfg);

        let mut f_cfg = p_cfg.firewall.clone().unwrap_or_default();
        f_cfg.enabled = Some(false);
        p_cfg.firewall = Some(f_cfg);

        config.pentester = Some(p_cfg);

        let saved_p = config.pentester.as_ref().unwrap();
        assert_eq!(saved_p.mode, Some(uneven_core::domain::PentesterMode::Active));
        assert_eq!(saved_p.static_config.as_ref().unwrap().owasp, Some(true));
        assert_eq!(saved_p.static_config.as_ref().unwrap().secrets, Some(false));
        assert_eq!(saved_p.bruteforce.as_ref().unwrap().enabled, Some(true));
        assert_eq!(saved_p.firewall.as_ref().unwrap().enabled, Some(false));
    }

    #[test]
    fn test_parse_shell_action_block() {
        let text = r#"
Here is my decision:
```shell-action
command: dde
args: [Add authentication route, with JWT]
paths: [src/routes/auth.rs, src/models/user.rs]
```
I have routed this to direct disk execution.
"#;
        let parsed = extract_json(text).expect("Should parse declarative shell-action block");
        assert_eq!(parsed.command, "dde");
        assert_eq!(parsed.args, vec!["Add authentication route".to_string(), "with JWT".to_string()]);
        assert_eq!(parsed.paths, vec!["src/routes/auth.rs".to_string(), "src/models/user.rs".to_string()]);
        assert!(parsed.message.contains("Here is my decision:"));
        assert!(parsed.message.contains("I have routed this to direct disk execution."));

        let intent = json_to_intent(&parsed);
        assert_eq!(intent.action, ShellAction::Askf);
    }

    #[test]
    fn test_parse_shell_action_block_think() {
        let text = r#"
```shell-action
command: think
args: Refactor auth service to use JWT tokens
paths: [src/auth.rs]
```
I am planning and auditing the implementation.
"#;
        let parsed = extract_json(text).expect("Should parse think shell-action block");
        assert_eq!(parsed.command, "think");
        let intent = json_to_intent(&parsed);
        assert_eq!(intent.action, ShellAction::Think);
        assert_eq!(intent.task, Some("Refactor auth service to use JWT tokens".to_string()));
    }

    #[test]
    fn test_classify_intent_think() {
        let input = "think implement rate limiting on /api/login";
        let intent = classify_intent(input);
        assert_eq!(intent.action, ShellAction::Think);
        assert_eq!(intent.task, Some("implement rate limiting on /api/login".to_string()));
    }
