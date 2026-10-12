import sqlite3

class LocalDatabaseManager:
    """Manages local SQLite database connections for tests."""
    def __init__(self, db_path: str = ":memory:"):
        self.conn = sqlite3.connect(db_path)

    def execute_query(self, sql: str, params: tuple = ()):
        cursor = self.conn.cursor()
        cursor.execute(sql, params)
        return cursor.fetchall()
