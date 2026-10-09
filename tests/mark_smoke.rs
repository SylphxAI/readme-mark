//! Render smoke: every form of the one grammar renders.

use mark::capabilities::mark::domain::art::Art;
use mark::capabilities::mark::domain::{MarkForm, MarkSpec};
use mark::capabilities::mark::render;

fn hero(art: &str, text: &str) -> MarkSpec {
    MarkSpec {
        form: MarkForm::Hero,
        art: Some(art.into()),
        text: Some(text.into()),
        ..Default::default()
    }
}

#[test]
fn hero_all_art_types_render() {
    for art in Art::ALL {
        let svg = render(&hero(art.id(), "T"));
        assert!(svg.starts_with("<?xml"), "type {}", art.id());
        assert!(svg.contains("</svg>"), "type {}", art.id());
    }
}

#[test]
fn hero_left_layout_anchors_a_large_social_card() {
    let spec = MarkSpec {
        form: MarkForm::Hero,
        art: Some("aurora".into()),
        theme: Some("grape".into()),
        animation: Some("none".into()),
        height: Some(640),
        width: Some(1280),
        text: Some("PDF Reader MCP".into()),
        desc: Some("The PDF intelligence layer".into()),
        hero: mark::capabilities::mark::domain::HeroSpec {
            layout: Some("left".into()),
        },
        ..Default::default()
    };
    let svg = render(&spec);
    assert!(svg.contains("text-anchor=\"start\""), "left-aligned titles");
    assert!(
        svg.contains("font-size=\"79\""),
        "type scales with the canvas"
    );
    assert!(svg.contains("PDF Reader MCP"));
}

#[test]
fn hero_type_reveals_once_then_rests() {
    let mut spec = hero("minimal", "Hi");
    spec.animation = Some("type".into());
    let svg = render(&spec);
    assert!(
        svg.contains("attributeName=\"width\""),
        "a sweep reveals the title"
    );
    assert!(!svg.contains("indefinite"), "minimal art never loops");
}

#[test]
fn hero_long_title_is_marked_inside_the_canvas() {
    let spec = MarkSpec {
        form: MarkForm::Hero,
        art: Some("soft".into()),
        animation: Some("none".into()),
        width: Some(360),
        height: Some(120),
        text: Some("A Very Long Display Name That Should Not Escape The Banner Canvas".into()),
        desc: Some("A similarly long tagline that also has to live inside the mark".into()),
        ..Default::default()
    };
    let svg = render(&spec);
    assert!(svg.contains('…'), "overflowing hero text is marked");
    assert!(
        !svg.contains("Should Not Escape The Banner Canvas"),
        "hero title must fit the canvas"
    );
    assert!(
        !svg.contains("also has to live inside the mark"),
        "hero desc must fit the canvas"
    );
}

#[test]
fn hero_short_title_is_not_truncated() {
    let svg = render(&hero("soft", "Ship it"));
    assert!(svg.contains("Ship it"));
    assert!(!svg.contains('…'));
}

#[test]
fn pill_styles_render() {
    for style in [
        "flat",
        "flat-square",
        "plastic",
        "for-the-badge",
        "social",
        "pill",
    ] {
        let spec = MarkSpec {
            form: MarkForm::Pill,
            pill: mark::capabilities::mark::domain::PillSpec {
                label: Some("build".into()),
                message: Some("passing".into()),
                style: Some(style.into()),
                ..Default::default()
            },
            ..Default::default()
        };
        let svg = render(&spec);
        assert!(
            svg.to_ascii_lowercase().contains("passing"),
            "style {style}"
        );
        assert!(svg.contains("<svg"));
    }
}

fn svg_width(svg: &str) -> u32 {
    svg.split("width=\"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

#[test]
fn pill_width_follows_glyph_advance() {
    let wide = render(&MarkSpec {
        form: MarkForm::Pill,
        pill: mark::capabilities::mark::domain::PillSpec {
            message: Some("WWWWWW".into()),
            ..Default::default()
        },
        ..Default::default()
    });
    let narrow = render(&MarkSpec {
        form: MarkForm::Pill,
        pill: mark::capabilities::mark::domain::PillSpec {
            message: Some("iiiiii".into()),
            ..Default::default()
        },
        ..Default::default()
    });
    let wide_w = svg_width(&wide);
    let narrow_w = svg_width(&narrow);
    assert!(
        wide_w > narrow_w,
        "wide glyphs must get a wider pill; WWWWWW={wide_w} iiiiii={narrow_w}"
    );
}

#[test]
fn deploy_width_follows_glyph_advance() {
    let wide = render(&MarkSpec {
        form: MarkForm::Deploy,
        deploy: mark::capabilities::mark::domain::DeploySpec {
            service: Some("WWWWWW".into()),
        },
        ..Default::default()
    });
    let narrow = render(&MarkSpec {
        form: MarkForm::Deploy,
        deploy: mark::capabilities::mark::domain::DeploySpec {
            service: Some("iiiiii".into()),
        },
        ..Default::default()
    });
    assert!(
        svg_width(&wide) > svg_width(&narrow),
        "the deploy pill measures glyphs, not characters: WWWWWW={} iiiiii={}",
        svg_width(&wide),
        svg_width(&narrow)
    );
}

fn strip_groups_are_balanced(svg: &str) {
    assert_eq!(
        svg.matches("<g").count(),
        svg.matches("</g>").count(),
        "strip SVG groups must be well-formed"
    );
}

fn strip_spec(anim: &str) -> MarkSpec {
    MarkSpec {
        form: MarkForm::Strip,
        theme: Some("dark".into()),
        animation: Some(anim.into()),
        strip: mark::capabilities::mark::domain::StripSpec {
            icons: Some("rust,ts,docker".into()),
            per_line: Some(8),
        },
        ..Default::default()
    }
}

#[test]
fn strip_new_catalog_icons_render() {
    let spec = MarkSpec {
        form: MarkForm::Strip,
        theme: Some("dark".into()),
        strip: mark::capabilities::mark::domain::StripSpec {
            icons: Some("java,terraform,mongodb,kotlin,swift".into()),
            per_line: Some(8),
        },
        ..Default::default()
    };
    let svg = render(&spec);
    assert!(svg.contains("<title>java</title>"));
    assert!(svg.contains("<title>terraform</title>"));
    assert!(
        !svg.contains(">?</text>"),
        "catalog ids must not fall back to the unknown tile"
    );
}

#[test]
fn strip_renders_and_caps() {
    let spec = MarkSpec {
        form: MarkForm::Strip,
        theme: Some("dark".into()),
        strip: mark::capabilities::mark::domain::StripSpec {
            icons: Some("rust,ts,docker,kubernetes".into()),
            per_line: Some(8),
        },
        ..Default::default()
    };
    let svg = render(&spec);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("rust"));
    strip_groups_are_balanced(&svg);
}

#[test]
fn strip_motion_wraps_the_icon_row() {
    let fade = render(&strip_spec("fade"));
    strip_groups_are_balanced(&fade);
    assert!(fade.contains("<animate"), "fade must emit SMIL");
    let rust = fade.find("<title>rust</title>").expect("rust icon");
    let wrap_close = fade.rfind("</g>").expect("row wrap");
    assert!(
        fade.find("<animate").expect("animate") < rust && rust < wrap_close,
        "fade SMIL must wrap the icon row, not an empty group"
    );

    let rise = render(&strip_spec("rise"));
    strip_groups_are_balanced(&rise);
    assert!(
        rise.contains("<animateTransform"),
        "rise must compose onto the strip"
    );

    let none = render(&strip_spec("none"));
    strip_groups_are_balanced(&none);
    assert!(!none.contains("<animate"), "static strip stays still");
}

#[test]
fn profile_renders_text_art_and_monogram() {
    let plain = render(&MarkSpec {
        form: MarkForm::Profile,
        text: Some("Kyle Tse".into()),
        ..Default::default()
    });
    assert!(plain.contains("Kyle Tse"));
    assert!(plain.contains(">KT<"), "profile owns a name monogram");
    let art = render(&MarkSpec {
        form: MarkForm::Profile,
        art: Some("aurora".into()),
        theme: Some("neon".into()),
        text: Some("Ada Lovelace".into()),
        desc: Some("AI-native platform".into()),
        ..Default::default()
    });
    assert!(art.contains("Ada Lovelace"));
    assert!(art.contains("AI-native platform"));
    assert!(art.contains(">AL<"));
    assert!(
        art.contains("clip-path=\"url(#mc)\""),
        "profile art is the hero's stage, clipped to the card"
    );
}

#[test]
fn profile_uses_native_geometry() {
    let svg = render(&MarkSpec {
        form: MarkForm::Profile,
        width: Some(320),
        height: Some(120),
        text: Some("Kyle Tse".into()),
        ..Default::default()
    });
    assert!(svg.contains("width=\"320\""), "profile honors width");
    assert!(svg.contains("height=\"120\""), "profile honors height");
    assert!(
        !svg.contains("scale("),
        "profile must compose at native geometry, not scale a 640 canvas"
    );
}

#[test]
fn identity_form_is_the_profile_card() {
    assert_eq!(MarkForm::parse(Some("identity")), MarkForm::Profile);
    let identity = render(&MarkSpec {
        form: MarkForm::parse(Some("identity")),
        text: Some("Ada Lovelace".into()),
        desc: Some("First programmer".into()),
        ..Default::default()
    });
    let profile = render(&MarkSpec {
        form: MarkForm::Profile,
        text: Some("Ada Lovelace".into()),
        desc: Some("First programmer".into()),
        ..Default::default()
    });
    assert_eq!(
        identity, profile,
        "retired identity form is the profile card"
    );
    assert!(identity.contains("Ada Lovelace"));
    assert!(identity.contains(">AL<"));
}

#[test]
fn profile_marks_overflowing_name() {
    let svg = render(&MarkSpec {
        form: MarkForm::Profile,
        width: Some(320),
        height: Some(120),
        text: Some("A Very Long Display Name That Should Not Escape The Card".into()),
        ..Default::default()
    });
    assert!(svg.contains('…'), "overflowing profile names are marked");
    assert!(
        !svg.contains("Should Not Escape The Card"),
        "profile name must fit the card"
    );
    assert!(
        svg.contains("clip-path=\"url(#pt)\""),
        "name column is clipped"
    );
}

#[test]
fn profile_marks_wide_glyph_overflow() {
    let svg = render(&MarkSpec {
        form: MarkForm::Profile,
        width: Some(320),
        height: Some(120),
        text: Some("WWWWWWWWWWWWWWWW".into()),
        desc: Some("MMMMMMMMMMMMMMMM".into()),
        ..Default::default()
    });
    assert!(
        svg.contains('…'),
        "wide glyphs must be marked, not clipped silently"
    );
    assert!(
        !svg.contains("WWWWWWWWWWWWWWWW"),
        "wide profile names must not be emitted in full"
    );
}

#[test]
fn profile_monogram_uses_non_latin_letters() {
    let cjk = render(&MarkSpec {
        form: MarkForm::Profile,
        text: Some("山田太郎".into()),
        ..Default::default()
    });
    assert!(cjk.contains(">山田<"), "CJK names own their monogram");
    assert!(
        !cjk.contains(">MK<"),
        "MK is not a stand-in for user letters"
    );
    let cyr = render(&MarkSpec {
        form: MarkForm::Profile,
        text: Some("Владимир".into()),
        ..Default::default()
    });
    assert!(cyr.contains(">ВЛ<"));
}

#[test]
fn profile_omits_empty_tagline() {
    let svg = render(&MarkSpec {
        form: MarkForm::Profile,
        text: Some("Kyle Tse".into()),
        ..Default::default()
    });
    assert!(svg.contains("Kyle Tse"));
}

#[test]
fn deploy_renders_conversion_pill() {
    let svg = render(&MarkSpec {
        form: MarkForm::Deploy,
        deploy: mark::capabilities::mark::domain::DeploySpec {
            service: Some("mark".into()),
        },
        ..Default::default()
    });
    assert!(svg.contains("deployed on"));
    assert!(svg.contains("mark · Sylphx"));
    assert!(
        svg.contains("<circle"),
        "deploy conversion mark owns a tile, not a generic two-rect pill"
    );
}

#[test]
fn composition_pill_motion_and_profile_art() {
    let pill = render(&MarkSpec {
        form: MarkForm::Pill,
        theme: Some("grape".into()),
        animation: Some("fade".into()),
        pill: mark::capabilities::mark::domain::PillSpec {
            label: Some("build".into()),
            message: Some("passing".into()),
            ..Default::default()
        },
        ..Default::default()
    });
    assert!(pill.contains("<animate"), "pill with motion composes");

    let profile = render(&MarkSpec {
        form: MarkForm::Profile,
        art: Some("waving".into()),
        theme: Some("ocean".into()),
        text: Some("Kyle Tse".into()),
        ..Default::default()
    });
    assert!(profile.contains("<svg"));
}

#[test]
fn mono_font_composes_into_hero_and_profile() {
    let mut spec = hero("transparent", "MCP & AI-agent tooling");
    spec.font = Some("mono".into());
    let hero_svg = render(&spec);
    assert!(hero_svg.contains("ui-monospace"), "hero mono font");
    let profile_svg = render(&MarkSpec {
        form: MarkForm::Profile,
        text: Some("Kyle Tse".into()),
        font: Some("mono".into()),
        ..Default::default()
    });
    assert!(profile_svg.contains("ui-monospace"), "profile mono font");
}

#[test]
fn mono_font_composes_into_pill_and_deploy() {
    let pill = render(&MarkSpec {
        form: MarkForm::Pill,
        font: Some("mono".into()),
        pill: mark::capabilities::mark::domain::PillSpec {
            label: Some("build".into()),
            message: Some("passing".into()),
            ..Default::default()
        },
        ..Default::default()
    });
    assert!(pill.contains("ui-monospace"), "pill mono font");
    let deploy = render(&MarkSpec {
        form: MarkForm::Deploy,
        font: Some("mono".into()),
        deploy: mark::capabilities::mark::domain::DeploySpec {
            service: Some("mark".into()),
        },
        ..Default::default()
    });
    assert!(deploy.contains("ui-monospace"), "deploy mono font");
}

#[test]
fn same_spec_renders_same_svg_forever() {
    let a = render(&hero("aurora", "Ship your release"));
    let b = render(&hero("aurora", "Ship your release"));
    assert_eq!(a, b, "determinism: same URL, same mark, forever");
}

#[test]
fn pill_and_deploy_paint_from_the_geometry_authority() {
    // Fixture: the shields text group (font family, size) and baselines have
    // exactly one owner (`domain/pill.rs`). A value shift fails this fixture.
    let flat = render(&MarkSpec {
        form: MarkForm::Pill,
        pill: mark::capabilities::mark::domain::PillSpec {
            label: Some("build".into()),
            message: Some("passing".into()),
            ..Default::default()
        },
        ..Default::default()
    });
    assert!(
        flat.contains("font-family=\"Verdana,Geneva,DejaVu Sans,sans-serif\"")
            && flat.contains("y=\"140\"")
            && flat.contains("textLength=\"270\""),
        "flat pill paints from the authority"
    );

    let badge = render(&MarkSpec {
        form: MarkForm::Pill,
        pill: mark::capabilities::mark::domain::PillSpec {
            label: Some("build".into()),
            message: Some("passing".into()),
            style: Some("for-the-badge".into()),
            ..Default::default()
        },
        ..Default::default()
    });
    assert!(
        badge.contains("font-size=\"100\"") && badge.contains("y=\"175\""),
        "badge paint comes from the authority"
    );

    let deploy = render(&MarkSpec {
        form: MarkForm::Deploy,
        ..Default::default()
    });
    assert!(
        deploy.contains("font-size=\"110\"") && deploy.contains("y=\"140\""),
        "deploy paints from the authority"
    );
}
