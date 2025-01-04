use crate::adapter::dal::Dal;
use crate::adapter::json::read_ndjson_file_and_create_bookmarks;
use crate::util::helper::calc_content_hash;
use crate::model::bookmark::BookmarkUpdater;
use anyhow::Context;
use camino::Utf8Path;
use tracing::debug;

pub fn create_embeddings_for_non_bookmarks<P>(file_path: P) -> anyhow::Result<()>
where
    P: AsRef<Utf8Path> + std::fmt::Display,
{
    // 1. read_ndjson_file_and_create_bookmarks
    let bms = read_ndjson_file_and_create_bookmarks(file_path)?;
    let mut dal = Dal::new(crate::CONFIG.db_url.clone());
    for mut bm in bms {
        debug!("Processing bookmark: {:?}", bm.convert_to_new_bookmark());

        match dal.get_bookmark_by_url(&bm.URL) {
            Ok(existing_bm) => {
                debug!("Existing bookmark: {:?}", existing_bm);
                bm.id = existing_bm.id; // make sure we have the correct id
                let new_hash = calc_content_hash(bm.get_content().as_str());

                if existing_bm.content_hash.is_some()
                    && existing_bm.content_hash != Some(new_hash.clone())
                {
                    debug!(
                        "Hashes differ, updating...: {:?} {:?}",
                        existing_bm.content_hash, new_hash
                    );
                    eprintln!("Hash different, updating text embedding: {:?}", bm.URL);
                    bm.update(); // create embeddings
                    bm.desc = "".to_string(); // we do not want the raw content in the db
                                              // todo:  changing this parameter type in method `update_bookmark` to borrow instead if owning the value
                    dal.update_bookmark(bm.clone())
                        .with_context(|| format!("Updating {:?}", bm))?;
                } else if existing_bm.content_hash.is_none() {
                    eprintln!("No hash found, create text embedding: {:?}", bm.URL);
                    bm.update(); // create embeddings
                    bm.desc = "".to_string(); // we do not want the raw content in the db
                    dal.update_bookmark(bm.clone())
                        .with_context(|| format!("Updating {:?}", bm))?;
                } else {
                    // hashes are the same
                    eprintln!("No change for: {:?}", bm.URL);
                }
            }
            Err(e) => {
                // Check if this is a NotFound error from diesel
                if e.to_string().contains("Record not found") {
                    eprintln!("Creating new text embedding: {:?}", bm.URL);
                    bm.update();
                    bm.desc = String::new();
                    dal.insert_bookmark(bm.convert_to_new_bookmark())?;
                } else {
                    return Err(e);
                }
            }
        }
    }
    Ok(())
}
