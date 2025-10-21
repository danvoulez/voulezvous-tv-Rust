use rusqlite::Connection;

pub fn configure_connection(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;\n\
         PRAGMA synchronous = NORMAL;\n\
         PRAGMA cache_size = -64000;\n\
         PRAGMA temp_store = MEMORY;\n\
         PRAGMA mmap_size = 30000000000;\n\
         PRAGMA busy_timeout = 5000;\n",
    )
}
