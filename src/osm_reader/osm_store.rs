// Copyright © 2016, Hove and/or its affiliates. All rights reserved.
//
// This file is part of Navitia,
//     the software to build cool stuff with public transport.
//
// Hope you'll enjoy and contribute to this project,
//     powered by Hove (www.kisio.com).
// Help us simplify mobility and open public transport:
//     a non ending quest to the responsive locomotion way of traveling!
//
// LICENCE: This program is free software; you can redistribute it
// and/or modify it under the terms of the GNU Affero General Public
// License as published by the Free Software Foundation, either
// version 3 of the License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
// Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public
// License along with this program. If not, see
// <http://www.gnu.org/licenses/>.
//
// Stay tuned using
// twitter @navitia
// IRC #navitia on freenode
// https://groups.google.com/d/forum/navitia
// www.navitia.io
#![allow(
    clippy::unused_unit,
    clippy::needless_return,
    clippy::never_loop,
    clippy::option_map_unit_fn
)]

#[cfg(feature = "db-storage")]
use {
    crate::settings::osm2mimir::Database,
    rusqlite::{Connection, DropBehavior, ToSql, Transaction},
    snafu::ResultExt,
    std::collections::HashSet,
    std::fs,
    std::path::{Path, PathBuf},
    tracing::error,
};

use osmpbfreader::{OsmId, OsmObj, StoreObjs};
use snafu::Snafu;
use tracing::info;

use std::{borrow::Cow, collections::BTreeMap};

use super::street::Kind;

#[cfg(feature = "db-storage")]
macro_rules! err_logger {
    ($obj:expr, $err_msg:expr) => {
        match $obj {
            Ok(x) => Some(x),
            Err(e) => {
                error!("{}: {}", $err_msg, e);
                None
            }
        }?
    };
    ($obj:expr, $err_msg:expr, $ret:expr) => {
        match $obj {
            Ok(x) => x,
            Err(e) => {
                error!("{}: {}", $err_msg, e);
                return $ret;
            }
        }
    };
}

fn obj_kind(id: OsmId) -> u8 {
    match id {
        OsmId::Node(_) => 0,
        OsmId::Way(_) => 1,
        OsmId::Relation(_) => 2,
    }
}

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("DB Storage Error: {}", msg))]
    DBStorage { msg: String },

    #[cfg(feature = "db-storage")]
    #[snafu(display("Sqlite Storage Error: {}", source))]
    SqliteStorage { source: rusqlite::Error },
}

pub trait Getter {
    fn get(&self, key: &OsmId) -> Option<Cow<OsmObj>>;
}

impl Getter for BTreeMap<OsmId, OsmObj> {
    fn get(&self, key: &OsmId) -> Option<Cow<OsmObj>> {
        self.get(key).map(Cow::Borrowed)
    }
}

#[cfg(feature = "db-storage")]
pub struct Db {
    conn: Connection,
    db_file: PathBuf,
}

#[cfg(feature = "db-storage")]
impl Db {
    fn new(db_file: &Path, db_cache_size: u32) -> Result<Db, Error> {
        let _ = fs::remove_file(db_file); // we ignore any potential error
        let conn = Connection::open(&db_file).context(SqliteStorageSnafu)?;

        conn.pragma_update(None, "page_size", &4096)
            .context(SqliteStorageSnafu)?;

        conn.pragma_update(None, "cache_size", &db_cache_size)
            .context(SqliteStorageSnafu)?;

        conn.pragma_update(None, "synchronous", &"OFF")
            .context(SqliteStorageSnafu)?;

        conn.pragma_update(None, "journal_mode", &"OFF")
            .context(SqliteStorageSnafu)?;

        conn.execute(
            "CREATE TABLE ids (
                id   INTEGER NOT NULL,
                obj  BLOB NOT NULL,
                kind INTEGER NOT NULL,
                UNIQUE(id, kind)
             )",
            [],
        )
        .context(SqliteStorageSnafu)?;

        Ok(Db {
            conn,
            db_file: db_file.into(),
        })
    }
}

#[cfg(feature = "db-storage")]
/// Wrapper around a transaction used for read access over the database.
pub struct DbReader<'d> {
    transaction: Transaction<'d>,
}

#[cfg(feature = "db-storage")]
impl<'d> DbReader<'d> {
    fn new(conn: &'d mut Connection) -> Result<Self, Error> {
        let transaction = conn.transaction().context(SqliteStorageSnafu)?;
        Ok(Self { transaction })
    }

    fn get_from_id(&self, id: &OsmId) -> Option<Cow<OsmObj>> {
        let mut stmt = err_logger!(
            self.transaction
                .prepare_cached("SELECT obj FROM ids WHERE id=?1 AND kind=?2"),
            "Db::get_from_id: prepare failed"
        );

        let obj: Vec<u8> = err_logger!(
            stmt.query_row(&[&id.inner_id() as &dyn ToSql, &obj_kind(*id)], |row| row
                .get(0)),
            "Db::get_from_id: query_map failed"
        );

        let obj: OsmObj = err_logger!(
            bincode::deserialize(&obj),
            "Db::for_each: serde conversion failed",
            None
        );

        Some(Cow::Owned(obj))
    }

    fn for_each<F: FnMut(Cow<OsmObj>)>(&self, mut f: F) {
        let mut stmt = err_logger!(
            self.transaction.prepare("SELECT obj FROM ids"),
            "Db::for_each: prepare failed",
            ()
        );
        let mut rows = err_logger!(stmt.query([]), "Db::for_each: query_map failed", ());
        while let Some(row) = err_logger!(rows.next(), "Db::for_each: next failed", ()) {
            let obj: Vec<u8> = err_logger!(row.get(0), "Db::for_each: failed to get obj field", ());

            let obj: OsmObj = err_logger!(
                bincode::deserialize(&obj),
                "Db::for_each: serde conversion failed",
                ()
            );
            f(Cow::Owned(obj));
        }
    }

    fn for_each_filter<F: FnMut(Cow<OsmObj>)>(&self, filter: Kind, mut f: F) {
        let mut stmt = err_logger!(
            self.transaction
                .prepare("SELECT obj FROM ids WHERE kind=?1"),
            "Db::for_each: prepare failed",
            ()
        );

        let mut rows = err_logger!(
            stmt.query(&[&(filter as u8) as &dyn ToSql]),
            "Db::for_each: query_map failed",
            ()
        );

        while let Some(row) = err_logger!(rows.next(), "Db::for_each: next failed", ()) {
            let obj: Vec<u8> = err_logger!(row.get(0), "Db::for_each: failed to get obj field", ());

            let obj: OsmObj = err_logger!(
                bincode::deserialize(&obj),
                "Db::for_each: serde conversion failed",
                ()
            );
            f(Cow::Owned(obj));
        }
    }
}

#[cfg(feature = "db-storage")]
impl Getter for DbReader<'_> {
    fn get(&self, key: &OsmId) -> Option<Cow<OsmObj>> {
        self.get_from_id(key)
    }
}

#[cfg(feature = "db-storage")]
/// Wrapper around a transaction used for write access into the database. This
/// also holds this history of inserted objects to avoid concurrent read / writes
/// on the database.
pub struct DbWritter<'d> {
    buffer_keys: HashSet<OsmId>,
    transaction: Transaction<'d>,
}

#[cfg(feature = "db-storage")]
impl<'d> DbWritter<'d> {
    fn new(conn: &'d mut Connection) -> Result<Self, Error> {
        let mut transaction = conn.transaction().context(SqliteStorageSnafu)?;
        transaction.set_drop_behavior(DropBehavior::Commit);

        Ok(Self {
            buffer_keys: HashSet::new(),
            transaction,
        })
    }

    fn insert(&mut self, id: OsmId, obj: OsmObj) {
        let ser_obj = err_logger!(
            bincode::serialize(&obj),
            "Db::insert: failed to convert to json",
            ()
        );

        let kind = obj_kind(id);

        let mut stmt_insert = err_logger!(
            self.transaction
                .prepare_cached("INSERT OR IGNORE INTO ids(id, obj, kind) VALUES (?1, ?2, ?3)"),
            "Db::insert: statement couldn't prepare",
            ()
        );

        err_logger!(
            stmt_insert.execute(&[&id.inner_id() as &dyn ToSql, &ser_obj, &kind]),
            "Db::insert failed",
            ()
        );
    }
}

#[cfg(feature = "db-storage")]
impl<'d> StoreObjs for DbWritter<'d> {
    fn insert(&mut self, id: OsmId, obj: OsmObj) {
        self.buffer_keys.insert(id);
        self.insert(id, obj);
    }

    fn contains_key(&self, id: &OsmId) -> bool {
        self.buffer_keys.contains(id)
    }
}

#[cfg(feature = "db-storage")]
impl Drop for Db {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.db_file); // we ignore any potential error
    }
}

#[cfg(feature = "db-storage")]
pub enum ObjWrapper {
    Map(BTreeMap<osmpbfreader::OsmId, osmpbfreader::OsmObj>),
    Db(Box<Db>),
}

#[cfg(not(feature = "db-storage"))]
pub enum ObjWrapper {
    Map(BTreeMap<osmpbfreader::OsmId, osmpbfreader::OsmObj>),
}

#[cfg(feature = "db-storage")]
impl ObjWrapper {
    pub fn new(db: Option<&Database>) -> Result<ObjWrapper, Error> {
        Ok(if let Some(db) = db {
            info!("Running with Db storage");
            ObjWrapper::Db(Box::new(Db::new(&db.file, db.cache_size)?))
        } else {
            info!("Running with BTreeMap (RAM) storage");
            ObjWrapper::Map(BTreeMap::new())
        })
    }

    pub fn get_reader(&mut self) -> Result<ObjReaderWrapper, Error> {
        Ok(match self {
            ObjWrapper::Map(map) => ObjReaderWrapper::Map(map),
            ObjWrapper::Db(db) => ObjReaderWrapper::Db(DbReader::new(&mut db.conn)?),
        })
    }

    pub fn get_writter(&mut self) -> Result<ObjWritterWrapper, Error> {
        Ok(match self {
            ObjWrapper::Map(map) => ObjWritterWrapper::Map(map),
            ObjWrapper::Db(db) => ObjWritterWrapper::Db(DbWritter::new(&mut db.conn)?),
        })
    }
}

#[cfg(not(feature = "db-storage"))]
impl ObjWrapper {
    pub fn new() -> Result<ObjWrapper, Error> {
        info!("Running with BTreeMap (RAM) storage");
        Ok(ObjWrapper::Map(BTreeMap::new()))
    }

    #[allow(dead_code)]
    pub fn for_each<F: FnMut(Cow<OsmObj>)>(&self, mut f: F) {
        match *self {
            ObjWrapper::Map(ref m) => {
                for value in m.values() {
                    f(Cow::Borrowed(value));
                }
            }
        }
    }

    pub fn for_each_filter<F: FnMut(Cow<OsmObj>)>(&self, filter: Kind, mut f: F) {
        match *self {
            ObjWrapper::Map(ref m) => {
                m.values()
                    .filter(|e| obj_kind(e.id()) == filter as u8)
                    .for_each(|value| f(Cow::Borrowed(value)));
            }
        }
    }
}

#[cfg(not(feature = "db-storage"))]
impl Getter for ObjWrapper {
    fn get(&self, key: &OsmId) -> Option<Cow<OsmObj>> {
        match *self {
            ObjWrapper::Map(ref m) => m.get(key).map(Cow::Borrowed),
        }
    }
}

#[cfg(not(feature = "db-storage"))]
impl StoreObjs for ObjWrapper {
    fn insert(&mut self, id: OsmId, obj: OsmObj) {
        match *self {
            ObjWrapper::Map(ref mut m) => {
                m.insert(id, obj);
            }
        }
    }

    fn contains_key(&self, id: &OsmId) -> bool {
        match *self {
            ObjWrapper::Map(ref m) => m.contains_key(id),
        }
    }
}

#[cfg(feature = "db-storage")]
pub enum ObjReaderWrapper<'d> {
    Map(&'d BTreeMap<osmpbfreader::OsmId, osmpbfreader::OsmObj>),
    Db(DbReader<'d>),
}

#[cfg(feature = "db-storage")]
impl<'d> ObjReaderWrapper<'d> {
    #[allow(dead_code)]
    pub fn for_each<F: FnMut(Cow<OsmObj>)>(&self, mut f: F) {
        match *self {
            ObjReaderWrapper::Map(m) => {
                for value in m.values() {
                    f(Cow::Borrowed(value));
                }
            }
            ObjReaderWrapper::Db(ref db) => db.for_each(f),
        }
    }

    pub fn for_each_filter<F: FnMut(Cow<OsmObj>)>(&self, filter: Kind, mut f: F) {
        match *self {
            ObjReaderWrapper::Map(m) => {
                m.values()
                    .filter(|e| obj_kind(e.id()) == filter as u8)
                    .for_each(|value| f(Cow::Borrowed(value)));
            }
            ObjReaderWrapper::Db(ref db) => db.for_each_filter(filter, f),
        }
    }
}

#[cfg(feature = "db-storage")]
impl<'d> Getter for ObjReaderWrapper<'d> {
    fn get(&self, key: &OsmId) -> Option<Cow<OsmObj>> {
        match *self {
            ObjReaderWrapper::Map(m) => m.get(key).map(|x| Cow::Borrowed(x)),
            ObjReaderWrapper::Db(ref db) => db.get(key),
        }
    }
}

#[cfg(feature = "db-storage")]
pub enum ObjWritterWrapper<'d> {
    Map(&'d mut BTreeMap<osmpbfreader::OsmId, osmpbfreader::OsmObj>),
    Db(DbWritter<'d>),
}

#[cfg(feature = "db-storage")]
impl<'d> StoreObjs for ObjWritterWrapper<'d> {
    fn insert(&mut self, id: OsmId, obj: OsmObj) {
        match *self {
            ObjWritterWrapper::Map(ref mut m) => {
                m.insert(id, obj);
            }
            ObjWritterWrapper::Db(ref mut db) => db.insert(id, obj),
        }
    }

    fn contains_key(&self, id: &OsmId) -> bool {
        match *self {
            ObjWritterWrapper::Map(ref m) => m.contains_key(id),
            ObjWritterWrapper::Db(ref db) => db.contains_key(id),
        }
    }
}
