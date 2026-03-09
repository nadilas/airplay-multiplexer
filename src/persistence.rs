use std::sync::Mutex;

use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::types::{RoomConfig, RoomId};

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open database at {}", path))?;
        let db = Self { conn: Mutex::new(conn) };
        db.initialize()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()
            .context("Failed to open in-memory database")?;
        let db = Self { conn: Mutex::new(conn) };
        db.initialize()?;
        Ok(db)
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("Database mutex poisoned")
    }

    fn initialize(&self) -> Result<()> {
        self.conn().execute_batch(
            "CREATE TABLE IF NOT EXISTS rooms (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                receiver_name TEXT NOT NULL,
                shairport_port INTEGER NOT NULL,
                is_default INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS device_assignments (
                device_id TEXT PRIMARY KEY NOT NULL,
                room_id TEXT NOT NULL,
                FOREIGN KEY (room_id) REFERENCES rooms(id) ON DELETE CASCADE
            );

            PRAGMA foreign_keys = ON;"
        ).context("Failed to initialize database schema")?;
        Ok(())
    }

    pub fn load_rooms(&self) -> Result<Vec<RoomConfig>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, name, receiver_name, shairport_port, is_default FROM rooms ORDER BY is_default DESC, name ASC"
        )?;

        let rooms = stmt.query_map([], |row| {
            Ok(RoomConfig {
                id: row.get(0)?,
                name: row.get(1)?,
                receiver_name: row.get(2)?,
                shairport_port: row.get::<_, u32>(3)? as u16,
                is_default: row.get::<_, i32>(4)? != 0,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("Failed to load rooms")?;

        Ok(rooms)
    }

    pub fn save_room(&self, room: &RoomConfig) -> Result<()> {
        self.conn().execute(
            "INSERT OR REPLACE INTO rooms (id, name, receiver_name, shairport_port, is_default) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                room.id,
                room.name,
                room.receiver_name,
                room.shairport_port as u32,
                room.is_default as i32,
            ],
        ).context("Failed to save room")?;
        Ok(())
    }

    pub fn delete_room(&self, room_id: &str) -> Result<()> {
        let conn = self.conn();
        conn.execute("DELETE FROM device_assignments WHERE room_id = ?1", [room_id])?;
        conn.execute("DELETE FROM rooms WHERE id = ?1", [room_id])
            .context("Failed to delete room")?;
        Ok(())
    }

    pub fn update_room_name(&self, room_id: &str, name: &str) -> Result<()> {
        self.conn().execute(
            "UPDATE rooms SET name = ?1 WHERE id = ?2",
            rusqlite::params![name, room_id],
        ).context("Failed to update room name")?;
        Ok(())
    }

    pub fn assign_device(&self, device_id: &str, room_id: &str) -> Result<()> {
        self.conn().execute(
            "INSERT OR REPLACE INTO device_assignments (device_id, room_id) VALUES (?1, ?2)",
            rusqlite::params![device_id, room_id],
        ).context("Failed to assign device")?;
        Ok(())
    }

    pub fn unassign_device(&self, device_id: &str) -> Result<()> {
        self.conn().execute(
            "DELETE FROM device_assignments WHERE device_id = ?1",
            [device_id],
        ).context("Failed to unassign device")?;
        Ok(())
    }

    pub fn get_device_room(&self, device_id: &str) -> Result<Option<RoomId>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT room_id FROM device_assignments WHERE device_id = ?1"
        )?;

        match stmt.query_row([device_id], |row| row.get(0)) {
            Ok(room_id) => Ok(Some(room_id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_room_device_ids(&self, room_id: &str) -> Result<Vec<String>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT device_id FROM device_assignments WHERE room_id = ?1"
        )?;

        let ids = stmt.query_map([room_id], |row| row.get(0))?
            .collect::<std::result::Result<Vec<String>, _>>()
            .context("Failed to get room device IDs")?;
        Ok(ids)
    }

    pub fn get_next_shairport_port(&self, base_port: u16) -> Result<u16> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT COALESCE(MAX(shairport_port), ?1 - 1) + 1 FROM rooms"
        )?;
        let port: u32 = stmt.query_row([base_port as u32], |row| row.get(0))?;
        Ok(port as u16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn test_empty_database() {
        let db = test_db();
        let rooms = db.load_rooms().unwrap();
        assert!(rooms.is_empty());
    }

    #[test]
    fn test_save_and_load_room() {
        let db = test_db();
        let room = RoomConfig {
            id: "living-room".to_string(),
            name: "Living Room".to_string(),
            receiver_name: "Living Room Audio".to_string(),
            shairport_port: 5100,
            is_default: true,
        };

        db.save_room(&room).unwrap();
        let rooms = db.load_rooms().unwrap();
        assert_eq!(rooms.len(), 1);
        assert_eq!(rooms[0].id, "living-room");
        assert_eq!(rooms[0].name, "Living Room");
        assert!(rooms[0].is_default);
    }

    #[test]
    fn test_delete_room() {
        let db = test_db();
        let room = RoomConfig {
            id: "bedroom".to_string(),
            name: "Bedroom".to_string(),
            receiver_name: "Bedroom Audio".to_string(),
            shairport_port: 5101,
            is_default: false,
        };

        db.save_room(&room).unwrap();
        assert_eq!(db.load_rooms().unwrap().len(), 1);

        db.delete_room("bedroom").unwrap();
        assert!(db.load_rooms().unwrap().is_empty());
    }

    #[test]
    fn test_update_room_name() {
        let db = test_db();
        let room = RoomConfig {
            id: "bedroom".to_string(),
            name: "Bedroom".to_string(),
            receiver_name: "Bedroom Audio".to_string(),
            shairport_port: 5101,
            is_default: false,
        };

        db.save_room(&room).unwrap();
        db.update_room_name("bedroom", "Guest Room").unwrap();

        let rooms = db.load_rooms().unwrap();
        assert_eq!(rooms[0].name, "Guest Room");
    }

    #[test]
    fn test_device_assignment() {
        let db = test_db();
        let room = RoomConfig {
            id: "living-room".to_string(),
            name: "Living Room".to_string(),
            receiver_name: "Living Room Audio".to_string(),
            shairport_port: 5100,
            is_default: true,
        };

        db.save_room(&room).unwrap();
        db.assign_device("sonos-192.168.1.10", "living-room").unwrap();

        let room_id = db.get_device_room("sonos-192.168.1.10").unwrap();
        assert_eq!(room_id, Some("living-room".to_string()));

        let device_ids = db.get_room_device_ids("living-room").unwrap();
        assert_eq!(device_ids, vec!["sonos-192.168.1.10"]);
    }

    #[test]
    fn test_unassign_device() {
        let db = test_db();
        let room = RoomConfig {
            id: "living-room".to_string(),
            name: "Living Room".to_string(),
            receiver_name: "Living Room Audio".to_string(),
            shairport_port: 5100,
            is_default: true,
        };

        db.save_room(&room).unwrap();
        db.assign_device("sonos-192.168.1.10", "living-room").unwrap();
        db.unassign_device("sonos-192.168.1.10").unwrap();

        let room_id = db.get_device_room("sonos-192.168.1.10").unwrap();
        assert!(room_id.is_none());
    }

    #[test]
    fn test_device_assignment_cascade_on_room_delete() {
        let db = test_db();
        let room = RoomConfig {
            id: "bedroom".to_string(),
            name: "Bedroom".to_string(),
            receiver_name: "Bedroom Audio".to_string(),
            shairport_port: 5101,
            is_default: false,
        };

        db.save_room(&room).unwrap();
        db.assign_device("airplay-192.168.1.20", "bedroom").unwrap();
        db.delete_room("bedroom").unwrap();

        let room_id = db.get_device_room("airplay-192.168.1.20").unwrap();
        assert!(room_id.is_none());
    }

    #[test]
    fn test_get_next_shairport_port() {
        let db = test_db();
        assert_eq!(db.get_next_shairport_port(5100).unwrap(), 5100);

        let room = RoomConfig {
            id: "room1".to_string(),
            name: "Room 1".to_string(),
            receiver_name: "Room 1".to_string(),
            shairport_port: 5100,
            is_default: true,
        };
        db.save_room(&room).unwrap();

        assert_eq!(db.get_next_shairport_port(5100).unwrap(), 5101);
    }

    #[test]
    fn test_reassign_device_to_different_room() {
        let db = test_db();
        let room1 = RoomConfig {
            id: "room1".to_string(),
            name: "Room 1".to_string(),
            receiver_name: "Room 1".to_string(),
            shairport_port: 5100,
            is_default: true,
        };
        let room2 = RoomConfig {
            id: "room2".to_string(),
            name: "Room 2".to_string(),
            receiver_name: "Room 2".to_string(),
            shairport_port: 5101,
            is_default: false,
        };

        db.save_room(&room1).unwrap();
        db.save_room(&room2).unwrap();

        db.assign_device("device-1", "room1").unwrap();
        db.assign_device("device-1", "room2").unwrap();

        let room_id = db.get_device_room("device-1").unwrap();
        assert_eq!(room_id, Some("room2".to_string()));

        let r1_devices = db.get_room_device_ids("room1").unwrap();
        assert!(r1_devices.is_empty());

        let r2_devices = db.get_room_device_ids("room2").unwrap();
        assert_eq!(r2_devices, vec!["device-1"]);
    }

    #[test]
    fn test_multiple_rooms_ordering() {
        let db = test_db();

        let room_b = RoomConfig {
            id: "bedroom".to_string(),
            name: "Bedroom".to_string(),
            receiver_name: "Bedroom".to_string(),
            shairport_port: 5102,
            is_default: false,
        };
        let room_a = RoomConfig {
            id: "living-room".to_string(),
            name: "Living Room".to_string(),
            receiver_name: "Living Room".to_string(),
            shairport_port: 5100,
            is_default: true,
        };
        let room_c = RoomConfig {
            id: "kitchen".to_string(),
            name: "Kitchen".to_string(),
            receiver_name: "Kitchen".to_string(),
            shairport_port: 5101,
            is_default: false,
        };

        db.save_room(&room_b).unwrap();
        db.save_room(&room_a).unwrap();
        db.save_room(&room_c).unwrap();

        let rooms = db.load_rooms().unwrap();
        assert_eq!(rooms.len(), 3);
        // Default room first, then alphabetical
        assert_eq!(rooms[0].id, "living-room");
        assert!(rooms[0].is_default);
    }
}
