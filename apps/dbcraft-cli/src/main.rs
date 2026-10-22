use clap::{Parser, Subcommand};
use dbcraft_core::ConnectionKind;
use dbcraft_engine::{CommandId, CommandParams, Session};
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(name = "dbcraft-cli", about = "DBCraft headless CLI", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// List saved connection profiles
    List,
    /// Create a SQLite profile and run a query (smoke test helper)
    Query {
        /// Connection profile id (UUID)
        #[arg(long)]
        connection: Option<Uuid>,
        /// SQL to execute
        #[arg(long)]
        sql: String,
        /// Create/use a temporary SQLite file profile instead of --connection
        #[arg(long)]
        sqlite: Option<String>,
    },
    /// Print schema tree for a connection as JSON
    Schema {
        #[arg(long)]
        connection: Uuid,
    },
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let cli = Cli::parse();
    let mut session = match Session::open() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to open session: {e}");
            std::process::exit(1);
        }
    };

    let code = match cli.command {
        Commands::List => {
            for p in session.profiles() {
                println!(
                    "{}\t{}\t{}",
                    p.id,
                    p.name,
                    p.kind.display_name()
                );
            }
            0
        }
        Commands::Query {
            connection,
            sql,
            sqlite,
        } => {
            let profile_id = if let Some(path) = sqlite {
                match session.dispatch(
                    CommandId::ConnectionCreate,
                    CommandParams {
                        name: Some("cli-sqlite".into()),
                        kind: Some(ConnectionKind::Sqlite),
                        file_path: Some(path),
                        ..Default::default()
                    },
                ) {
                    Ok(_) => session.profiles().last().map(|p| p.id),
                    Err(e) => {
                        eprintln!("{e}");
                        None
                    }
                }
            } else {
                connection
            };
            let Some(id) = profile_id else {
                eprintln!("provide --connection <uuid> or --sqlite <path>");
                std::process::exit(2);
            };
            if let Err(e) = session.dispatch(
                CommandId::ConnectionConnect,
                CommandParams {
                    profile_id: Some(id),
                    ..Default::default()
                },
            ) {
                eprintln!("connect failed: {e}");
                std::process::exit(1);
            }
            if let Err(e) = session.dispatch(
                CommandId::QueryExecute,
                CommandParams {
                    sql: Some(sql),
                    ..Default::default()
                },
            ) {
                eprintln!("query failed: {e}");
                std::process::exit(1);
            }
            if let Some(result) = session.last_result() {
                match serde_json::to_string_pretty(result) {
                    Ok(json) => println!("{json}"),
                    Err(e) => eprintln!("{e}"),
                }
            }
            0
        }
        Commands::Schema { connection } => {
            if let Err(e) = session.dispatch(
                CommandId::ConnectionConnect,
                CommandParams {
                    profile_id: Some(connection),
                    ..Default::default()
                },
            ) {
                eprintln!("connect failed: {e}");
                std::process::exit(1);
            }
            match serde_json::to_string_pretty(session.schema_tree()) {
                Ok(json) => {
                    println!("{json}");
                    0
                }
                Err(e) => {
                    eprintln!("{e}");
                    1
                }
            }
        }
    };
    let _ = session.flush();
    std::process::exit(code);
}
