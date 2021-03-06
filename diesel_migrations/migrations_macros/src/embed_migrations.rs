use crate::migrations::migration_directory_from_given_path;
use migrations_internals::{migration_paths_in_directory, version_from_path};
use quote::quote;
use std::error::Error;
use std::fs::DirEntry;
use std::path::Path;

pub fn expand(path: String) -> proc_macro2::TokenStream {
    let migrations_path_opt = if path.is_empty() {
        None
    } else {
        Some(path.replace("\"", ""))
    };
    let migrations_expr =
        migration_directory_from_given_path(migrations_path_opt.as_ref().map(String::as_str))
            .and_then(|path| migration_literals_from_path(&path));
    let migrations_expr = match migrations_expr {
        Ok(v) => v,
        Err(e) => panic!("Error reading migrations: {}", e),
    };

    quote! {
        #[allow(dead_code)]
        mod embedded_migrations {
            extern crate diesel;
            extern crate diesel_migrations;

            use self::diesel_migrations::*;
            use self::diesel::connection::SimpleConnection;
            use std::io;

            const ALL_MIGRATIONS: &[&Migration] = &[#(#migrations_expr),*];

            struct EmbeddedMigration {
                version: &'static str,
                up_sql: &'static str,
            }

            impl Migration for EmbeddedMigration {
                fn version(&self) -> &str {
                    self.version
                }

                fn run(&self, conn: &SimpleConnection) -> Result<(), RunMigrationsError> {
                    conn.batch_execute(self.up_sql).map_err(Into::into)
                }

                fn revert(&self, _conn: &SimpleConnection) -> Result<(), RunMigrationsError> {
                    unreachable!()
                }
            }

            pub fn run<C: MigrationConnection>(conn: &C) -> Result<(), RunMigrationsError> {
                run_with_output(conn, &mut io::sink())
            }

            pub fn run_with_output<C: MigrationConnection>(
                conn: &C,
                out: &mut io::Write,
            ) -> Result<(), RunMigrationsError> {
                run_migrations(conn, ALL_MIGRATIONS.iter().map(|v| *v), out)
            }
        }
    }
}

fn migration_literals_from_path(
    path: &Path,
) -> Result<Vec<proc_macro2::TokenStream>, Box<dyn Error>> {
    let mut migrations = migration_paths_in_directory(path)?;

    migrations.sort_by_key(DirEntry::path);

    migrations
        .into_iter()
        .map(|e| migration_literal_from_path(&e.path()))
        .collect()
}

fn migration_literal_from_path(path: &Path) -> Result<proc_macro2::TokenStream, Box<dyn Error>> {
    let version = version_from_path(path)?;
    let sql_file = path.join("up.sql");
    let sql_file_path = sql_file.to_str();

    Ok(quote!(&EmbeddedMigration {
        version: #version,
        up_sql: include_str!(#sql_file_path),
    }))
}
