use cli_command_kit::Mode;
use std::ffi::OsString;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn canonical_skill_matches_the_native_interface_and_compiled_pointer() {
    let guide = include_str!("../skills/gws-cache/SKILL.md");
    let omissions = cli_command_kit::skill_omissions(
        &gws_cache_cli::args::command(Mode::Agent),
        &gws_cache_cli::args::help_options(),
        guide,
    );
    assert!(omissions.is_empty(), "{omissions:#?}");
    assert!(guide.contains("name: gws-cache\n"));
    for mode in [Mode::Human, Mode::Agent] {
        let mut out = Vec::new();
        assert_eq!(
            gws_cache_cli::entry(
                ["gws-cache", "skill"].map(OsString::from),
                &mut out,
                &mut Vec::new(),
                mode,
                &CancellationToken::new()
            )
            .await,
            0
        );
        assert_eq!(out, gws_cache_cli::args::INSTALL.as_bytes());
    }
}

#[tokio::test]
async fn bare_root_displays_help_successfully() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    assert_eq!(
        gws_cache_cli::entry(
            [OsString::from("gws-cache")],
            &mut out,
            &mut err,
            Mode::Human,
            &CancellationToken::new()
        )
        .await,
        0
    );
    assert!(err.is_empty());
    assert!(String::from_utf8(out).unwrap().contains("gws-cache"));
}

#[tokio::test]
async fn all_help_paths_are_agent_aware_and_never_open_the_cache() {
    for mode in [Mode::Human, Mode::Agent] {
        let paths = [
            vec![],
            vec!["topic"],
            vec!["topic", "add"],
            vec!["topic", "list"],
            vec!["topic", "rm"],
            vec!["message"],
            vec!["message", "sync"],
            vec!["message", "search"],
            vec!["message", "export"],
            vec!["event"],
            vec!["event", "sync"],
            vec!["event", "search"],
            vec!["cache"],
            vec!["cache", "status"],
            vec!["skill"],
            vec!["completion"],
            vec!["help"],
            vec!["help", "message", "search"],
        ];
        for path in paths {
            let mut argv = vec![OsString::from("gws-cache")];
            argv.extend(path.iter().map(OsString::from));
            if path.first() != Some(&"help") {
                argv.push("--help".into());
            }
            let mut out = Vec::new();
            let mut err = Vec::new();
            let result =
                gws_cache_cli::entry(argv, &mut out, &mut err, mode, &CancellationToken::new())
                    .await;
            assert_eq!(result, 0, "{path:?}: {}", String::from_utf8_lossy(&err));
            assert!(err.is_empty());
            let text = String::from_utf8(out).unwrap();
            assert!(text.contains("gws-cache"));
            if mode == Mode::Agent {
                assert!(text.contains("AGENT=1 gws-cache skill"), "{path:?}: {text}");
            }
        }
    }
}

#[tokio::test]
async fn cancellation_and_broken_writers_are_reported_without_panics() {
    struct Broken;
    impl std::io::Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("closed output"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    for args in [
        vec!["--version"],
        vec!["--help"],
        vec!["skill"],
        vec!["completion", "fish"],
    ] {
        let mut argv = vec![OsString::from("gws-cache")];
        argv.extend(args.into_iter().map(OsString::from));
        assert_eq!(
            gws_cache_cli::entry(
                argv,
                &mut Broken,
                &mut Vec::new(),
                Mode::Human,
                &CancellationToken::new()
            )
            .await,
            1
        );
    }
    let cancel = CancellationToken::new();
    cancel.cancel();
    let mut err = Vec::new();
    assert_eq!(
        gws_cache_cli::entry(
            ["gws-cache", "cache", "status"].map(OsString::from),
            &mut Vec::new(),
            &mut err,
            Mode::Human,
            &cancel
        )
        .await,
        130
    );
    assert!(String::from_utf8(err).unwrap().starts_with("[ERR]"));
}
