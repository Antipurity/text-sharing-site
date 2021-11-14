//! Stores posts.
//! This is a database-less thunk that simply stores posts in memory.
//!   TODO: Use Firebase, not this thunk.



use crate::posts_api::Post;

use std::sync::RwLock;
use std::collections::HashMap;

use chrono::Datelike;



pub struct Database {
    posts: RwLock<HashMap<String, Post>>,
    access_hash_to_first_post_id: RwLock<HashMap<String, String>>,
    human_readable_url: RwLock<HashMap<String, String>>,
}



impl Database {
    /// Initializes the database connection.
    pub fn new() -> Database {
        Database{
            posts: RwLock::new(HashMap::new()),
            access_hash_to_first_post_id: RwLock::new(HashMap::new()),
            human_readable_url: RwLock::new(HashMap::new()),
        }
    }
    /// Reads many posts from the database at once.
    pub fn read(&self, ids: Vec<&str>) -> Vec<Option<Post>> {
        let map = self.posts.read().unwrap();
        ids.iter().map(|id| {
            let maybe_post = (*map).get(id.clone());
            maybe_post.map(|post| (*post).clone())
        }).collect()
    }
    /// Updates many posts in the database at once: read, process, write, as one atomic operation.
    pub fn update<F>(&self, ids: Vec<&str>, action: F)
    where F: FnOnce(Vec<Option<Post>>) -> Vec<Option<Post>> {
        let posts = self.read(ids);
        let posts = action(posts);
        let mut map_lock = self.posts.write().unwrap();
        let mut login_lock = self.access_hash_to_first_post_id.write().unwrap();
        let mut human_lock = self.human_readable_url.write().unwrap();
        let map = &mut *map_lock;
        let login = &mut *login_lock;
        let human = &mut *human_lock;
        for maybe_post in posts {
            if let Some(mut post) = maybe_post {
                let key = post.id.clone();
                if !login.contains_key(&post.access_hash) { // Update login info too.
                    login.insert(post.access_hash.clone(), key.clone());
                }
                if post.human_readable_url == "" {
                    post.human_readable_url = to_url_part(&post.content)
                }
                if !human.contains_key(&post.human_readable_url) {
                    human.insert(post.human_readable_url.clone(), post.id.clone());
                }
                map.insert(key, post);
            }
        }
    }

    /// Authenticates a user's access token (username+password hashed), returning the first-post ID if there is such a user registered, else `None`.
    pub fn login(&self, user: &str) -> Option<String> {
        let login_lock = self.access_hash_to_first_post_id.read().unwrap();
        let login = &*login_lock;
        return login.get(&crate::posts_api::access_token_hash(user)).map(|s| s.clone())
    }
    /// Converts a human-readable URL to the post ID, if present in the database.
    /// To get a post's URL, read `post.human_readable_url`: an empty string if not assigned.
    /// These URLs are auto-assigned, and will never collide with raw post IDs, nor with statically-served files (since these URLs are like `"2020_first_line_of_content"`).
    pub fn lookup_url(&self, url: &str) -> Option<String> {
        let mut human_lock = self.human_readable_url.write().unwrap();
        let human = &mut *human_lock;
        human.get(url.clone()).map(|s| s.clone())
    }
}



fn to_url_part(content: &str) -> String {
    let year = chrono::Utc::now().year().to_string();
    let simpler = content.replace(|c:char| !c.is_ascii(), "_");
    let simpler = simpler.split('_').filter(|s| s.len() > 0).collect::<Vec<&str>>().join("_");
    let simpler = simpler.to_ascii_lowercase();
    year + "_" + if simpler.len() < 80 { &simpler } else { &simpler[..80] }
}