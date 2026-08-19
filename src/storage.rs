use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Local};
use rusqlite::{Connection, params};

use crate::model::Todo;

pub struct TodoStore {
    conn: Connection,
    data: Vec<Todo>,
}

impl TodoStore {
    pub fn load() -> Result<Self> {
        let path = data_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("无法创建 {}", parent.display()))?;
        }
        Self::open(path)
    }

    fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path.as_ref())
            .with_context(|| format!("无法打开数据库 {}", path.as_ref().display()))?;
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS todos (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 title TEXT NOT NULL,
                 completed INTEGER NOT NULL DEFAULT 0,
                 created_at TEXT NOT NULL,
                 completed_at TEXT
             );
             CREATE INDEX IF NOT EXISTS idx_todos_completed_created
                 ON todos (completed, created_at);",
        )
        .context("无法初始化 Todo 数据表")?;
        let data = load_items(&conn)?;
        Ok(Self { conn, data })
    }

    pub fn items(&self) -> &[Todo] {
        &self.data
    }

    pub fn add(&mut self, title: String) -> Result<&Todo> {
        let title = title.trim().to_owned();
        if title.is_empty() {
            bail!("待办内容不能为空");
        }

        let created_at = Local::now();
        self.conn
            .execute(
                "INSERT INTO todos (title, completed, created_at) VALUES (?1, 0, ?2)",
                params![title, created_at.to_rfc3339()],
            )
            .context("无法新增待办")?;
        let id = self.conn.last_insert_rowid() as u64;
        self.data.push(Todo {
            id,
            title,
            completed: false,
            created_at,
            completed_at: None,
        });
        Ok(self.data.last().expect("刚刚插入的待办应当存在"))
    }

    pub fn toggle(&mut self, index: usize) -> Result<()> {
        let item = self.data.get_mut(index).context("找不到选中的待办")?;
        let completed = !item.completed;
        let completed_at = completed.then(Local::now);
        self.conn
            .execute(
                "UPDATE todos SET completed = ?1, completed_at = ?2 WHERE id = ?3",
                params![
                    completed as i64,
                    completed_at.as_ref().map(DateTime::to_rfc3339),
                    item.id as i64
                ],
            )
            .context("无法更新待办状态")?;
        item.completed = completed;
        item.completed_at = completed_at;
        Ok(())
    }

    pub fn update_title(&mut self, index: usize, title: String) -> Result<()> {
        let title = title.trim().to_owned();
        if title.is_empty() {
            bail!("待办内容不能为空");
        }
        let item = self.data.get_mut(index).context("找不到选中的待办")?;
        self.conn
            .execute(
                "UPDATE todos SET title = ?1 WHERE id = ?2",
                params![title, item.id as i64],
            )
            .context("无法修改待办内容")?;
        item.title = title;
        Ok(())
    }

    pub fn reload(&mut self) -> Result<()> {
        self.data = load_items(&self.conn)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_by_id(&self, id: u64) -> Option<&Todo> {
        self.data.iter().find(|item| item.id == id)
    }

    pub fn find_index_by_id(&self, id: u64) -> Option<usize> {
        self.data.iter().position(|item| item.id == id)
    }

    pub fn set_completed_by_id(&mut self, id: u64, completed: bool) -> Result<&Todo> {
        let index = self
            .find_index_by_id(id)
            .with_context(|| format!("找不到编号为 #{id} 的待办事项"))?;
        if self.data[index].completed == completed {
            return Ok(&self.data[index]);
        }
        let completed_at = completed.then(Local::now);
        self.conn
            .execute(
                "UPDATE todos SET completed = ?1, completed_at = ?2 WHERE id = ?3",
                params![
                    completed as i64,
                    completed_at.as_ref().map(DateTime::to_rfc3339),
                    id as i64
                ],
            )
            .with_context(|| format!("无法更新待办 #{id} 状态"))?;
        let item = &mut self.data[index];
        item.completed = completed;
        item.completed_at = completed_at;
        Ok(&self.data[index])
    }

    pub fn update_title_by_id(&mut self, id: u64, title: String) -> Result<&Todo> {
        let title = title.trim().to_owned();
        if title.is_empty() {
            bail!("待办内容不能为空");
        }
        let index = self
            .find_index_by_id(id)
            .with_context(|| format!("找不到编号为 #{id} 的待办事项"))?;
        self.conn
            .execute(
                "UPDATE todos SET title = ?1 WHERE id = ?2",
                params![title, id as i64],
            )
            .with_context(|| format!("无法修改待办 #{id} 内容"))?;
        self.data[index].title = title;
        Ok(&self.data[index])
    }

    pub fn remove_by_id(&mut self, id: u64) -> Result<Todo> {
        let index = self
            .find_index_by_id(id)
            .with_context(|| format!("找不到编号为 #{id} 的待办事项"))?;
        self.conn
            .execute("DELETE FROM todos WHERE id = ?1", params![id as i64])
            .with_context(|| format!("无法删除待办 #{id}"))?;
        Ok(self.data.remove(index))
    }

    pub fn clear_completed(&mut self) -> Result<usize> {
        let count = self
            .conn
            .execute("DELETE FROM todos WHERE completed = 1", [])
            .context("无法清理已完成待办")?;
        self.data.retain(|item| !item.completed);
        Ok(count)
    }

    pub fn search(&self, keyword: &str) -> Vec<&Todo> {
        let kw = keyword.trim().to_lowercase();
        if kw.is_empty() {
            return self.data.iter().collect();
        }
        self.data
            .iter()
            .filter(|item| item.title.to_lowercase().contains(&kw) || item.id.to_string() == kw)
            .collect()
    }

    pub fn remove(&mut self, index: usize) -> Result<()> {
        let item = self.data.get(index).context("找不到选中的待办")?;
        self.conn
            .execute("DELETE FROM todos WHERE id = ?1", params![item.id as i64])
            .context("无法删除待办")?;
        self.data.remove(index);
        Ok(())
    }
}

fn load_items(conn: &Connection) -> Result<Vec<Todo>> {
    let mut statement = conn
        .prepare(
            "SELECT id, title, completed, created_at, completed_at
             FROM todos ORDER BY id ASC",
        )
        .context("无法查询待办列表")?;
    let rows = statement
        .query_map([], |row| {
            let id: i64 = row.get(0)?;
            let title = row.get(1)?;
            let completed: i64 = row.get(2)?;
            let created_at: String = row.get(3)?;
            let completed_at: Option<String> = row.get(4)?;
            Ok((id, title, completed, created_at, completed_at))
        })
        .context("无法读取待办列表")?;

    rows.map(|row| {
        let (id, title, completed, created_at, completed_at) = row.context("无法读取待办记录")?;
        let created_at = parse_time(&created_at)?;
        let completed_at = completed_at.as_deref().map(parse_time).transpose()?;
        Ok(Todo {
            id: u64::try_from(id).context("待办编号超出支持范围")?,
            title,
            completed: completed != 0,
            created_at,
            completed_at,
        })
    })
    .collect()
}

fn parse_time(value: &str) -> Result<DateTime<Local>> {
    DateTime::parse_from_rfc3339(value)
        .with_context(|| format!("无法解析时间：{value}"))
        .map(|time| time.with_timezone(&Local))
}

fn data_path() -> Result<PathBuf> {
    let base = dirs::data_dir().context("无法确定用户数据目录")?;
    Ok(base.join("todo").join("todos.db"))
}

#[cfg(test)]
mod tests {
    use super::*;

    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn test_store(name: &str) -> (TodoStore, PathBuf) {
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "todo-test-{}-{}-{}.db",
            std::process::id(),
            name,
            id
        ));
        let _ = fs::remove_file(&path);
        let store = TodoStore::open(&path).expect("测试数据库应当可以打开");
        (store, path)
    }

    #[test]
    fn empty_title_is_rejected() {
        let (mut store, path) = test_store("empty");
        assert!(store.add("  ".to_owned()).is_err());
        drop(store);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn persists_add_toggle_and_remove() {
        let (mut store, path) = test_store("persist");
        let item = store.add("测试 SQLite".to_owned()).expect("新增应成功");
        assert_eq!(item.id, 1);
        assert_eq!(store.items().len(), 1);

        // 修改测试
        store
            .update_title(0, "修改后的标题".to_owned())
            .expect("修改应成功");
        assert_eq!(store.items()[0].title, "修改后的标题");

        store.toggle(0).expect("完成应成功");
        assert!(store.items()[0].completed);
        drop(store);

        let mut reopened = TodoStore::open(&path).expect("数据库应当可以重新打开");
        assert_eq!(reopened.items()[0].title, "修改后的标题");
        assert!(reopened.items()[0].completed);
        reopened.remove(0).expect("删除应成功");
        assert!(reopened.items().is_empty());
        drop(reopened);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn reloads_external_changes() {
        let (_, path) = test_store("reload");
        let mut store1 = TodoStore::open(&path).expect("数据库应当可以打开");
        let mut store2 = TodoStore::open(&path).expect("数据库应当可以打开");

        store1.add("由实例1添加".to_owned()).expect("添加应成功");
        assert_eq!(store1.items().len(), 1);
        assert_eq!(store2.items().len(), 0);

        store2.reload().expect("重载应成功");
        assert_eq!(store2.items().len(), 1);
        assert_eq!(store2.items()[0].title, "由实例1添加");

        drop(store1);
        drop(store2);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn id_based_crud_search_and_clear() {
        let (mut store, path) = test_store("crud");
        let t1 = store.add("任务一".to_owned()).expect("添加1");
        let id1 = t1.id;
        let t2 = store.add("任务二(待办)".to_owned()).expect("添加2");
        let id2 = t2.id;

        assert_eq!(store.get_by_id(id1).unwrap().title, "任务一");
        assert_eq!(store.search("二").len(), 1);
        assert_eq!(store.search("二")[0].id, id2);

        // 修改标题
        store
            .update_title_by_id(id1, "更新后的任务一".to_owned())
            .expect("修改标题");
        assert_eq!(store.get_by_id(id1).unwrap().title, "更新后的任务一");

        // 标记完成
        store.set_completed_by_id(id1, true).expect("标记完成");
        assert!(store.get_by_id(id1).unwrap().completed);

        // 撤销完成
        store.set_completed_by_id(id1, false).expect("撤销完成");
        assert!(!store.get_by_id(id1).unwrap().completed);

        // 再次完成并测试 clear_completed
        store.set_completed_by_id(id1, true).expect("标记完成");
        let cleared = store.clear_completed().expect("清理完成待办");
        assert_eq!(cleared, 1);
        assert_eq!(store.items().len(), 1);
        assert_eq!(store.items()[0].id, id2);

        // 按 ID 删除
        store.remove_by_id(id2).expect("按 ID 删除");
        assert!(store.items().is_empty());
        drop(store);
        let _ = fs::remove_file(path);
    }
}
