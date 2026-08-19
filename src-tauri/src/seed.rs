//! First-run seed data: the 14 catalog entries from
//! legacy/config/catalog.yaml (kept as the runnable parity reference until
//! v1 release). Seeded once into the Library; every row is deletable.

use crate::domain::{EnvAction, EnvWiring, Product};

fn env(action: EnvAction, name: &str, value: &str) -> EnvWiring {
    EnvWiring {
        action,
        name: name.to_string(),
        value: value.to_string(),
    }
}

fn product(
    id: &str,
    name: &str,
    winget_id: Option<&str>,
    install_location_hint: Option<&str>,
    install_dir: None,
    default_env: Vec<EnvWiring>,
) -> Product {
    Product {
        id: id.to_string(),
        name: name.to_string(),
        winget_id: winget_id.map(str::to_string),
        install_location_hint: install_location_hint.map(str::to_string),
        install_dir: None,
        default_env,
    }
}

/// The 14 seeded Products (asserted in tests; informational in the app).
#[allow(dead_code)]
pub const SEED_COUNT: usize = 14;

/// The 14 seeded Products, mirrored from the legacy catalog 1:1 (ids, names,
/// winget ids, hints, and default env wiring).
pub fn seed_products() -> Vec<Product> {
    vec![
        product("dbeaver", "DBeaver", Some("DBeaver.DBeaver.Community"), None, vec![]),
        product(
            "openjdk21",
            "Eclipse Temurin OpenJDK 21 (LTS)",
            Some("EclipseAdoptium.Temurin.21.JDK"),
            Some("Eclipse Temurin"),
            vec![
                env(EnvAction::Set, "JAVA_HOME", "<InstallLocation:Eclipse Temurin>"),
                env(
                    EnvAction::Prepend,
                    "PATH",
                    "<InstallLocation:Eclipse Temurin>\\bin",
                ),
            ],
        ),
        product("git", "Git", Some("Git.Git"), None, vec![]),
        product("docker", "Docker Desktop", Some("Docker.DockerDesktop"), None, vec![]),
        product(
            "redis-manager",
            "Another Redis Desktop Manager",
            Some("qishibo.AnotherRedisDesktopManager"),
            None,
            vec![],
        ),
        product("postman", "Postman", Some("Postman.Postman"), None, vec![]),
        product(
            "mongodb-compass",
            "MongoDB Compass",
            Some("MongoDB.Compass.Full"),
            None,
            vec![],
        ),
        product("sourcetree", "SourceTree", Some("Atlassian.Sourcetree"), None, vec![]),
        product(
            "mysql-workbench",
            "MySQL Workbench",
            Some("Oracle.MySQLWorkbench"),
            None,
            vec![],
        ),
        product(
            "vscode",
            "Visual Studio Code",
            Some("Microsoft.VisualStudioCode"),
            None,
            vec![],
        ),
        product(
            "vscommunity",
            "Visual Studio Community",
            Some("Microsoft.VisualStudio.Community"),
            None,
            vec![],
        ),
        product(
            "intellij",
            "IntelliJ IDEA Community",
            Some("JetBrains.IntelliJIDEA"),
            None,
            vec![],
        ),
        product(
            "nvm",
            "NVM for Windows",
            Some("CoreyButler.NVMforWindows"),
            Some("NVM for Windows"),
            vec![env(EnvAction::Prepend, "PATH", "<InstallLocation:NVM for Windows>")],
        ),
        // Legacy custom step (nvm install lts) — not winget-managed.
        product("node-lts", "Node.js LTS (via NVM)", None, None, vec![]),
    ]
}
