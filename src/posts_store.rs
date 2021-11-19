//! Stores posts.
//! This is a database-less thunk that simply stores posts in memory.
//!   TODO: Use Firebase, not this thunk.
//!   TODO: Collections that we want: posts; access_hash_to_first_post_id; human_readable_url.



use crate::posts_api::Post;

use std::sync::RwLock;
use std::collections::HashMap;

use chrono::Datelike;
use firebase_rs::Firebase;



pub struct Database {
    pub firebase: Firebase, // I guess this doesn't need our synchronization.
    posts: RwLock<HashMap<String, Post>>,
    access_hash_to_first_post_id: RwLock<HashMap<String, String>>,
    human_readable_url: RwLock<HashMap<String, String>>,
}



impl Database {
    /// Initializes the database connection.
    pub fn new(fb: Firebase) -> Database {
        Database{
            firebase: fb,
            posts: RwLock::new(HashMap::new()),
            access_hash_to_first_post_id: RwLock::new(HashMap::new()),
            human_readable_url: RwLock::new(HashMap::new()),
        }
    }
    /// Reads many posts from the database at once.
    pub fn read(&self, ids: Vec<&str>) -> Vec<Option<Post>> {
        let fb = &self.firebase; // TODO:
        fb.at("hello").unwrap().set("15").unwrap(); // TODO: Use these calls to get data. Remove `.posts`.
        let map = self.posts.read().unwrap(); // TODO: Don't use `map`, use `.firebase`.
        ids.iter().map(|id| {
            let maybe_post = (*map).get(id.clone()); // TODO: Don't use `map`, use `.firebase`.
            maybe_post.map(|post| (*post).clone())
        }).collect()
    }
    /// Updates many posts in the database at once: read, process, write, as one atomic operation.
    /// (Well, "atomic" is a word too strong for this: it first reads all, then writes all, each of these being atomic. Who cares about race conditions in this simple site?)
    pub fn update<F>(&self, ids: Vec<&str>, action: F)
    where F: FnOnce(Vec<Option<Post>>) -> Vec<Option<Post>> {
        let posts = self.read(ids);
        let posts = action(posts);
        let mut map_lock = self.posts.write().unwrap(); // TODO: Don't use `map`, use `.firebase`.
        let mut login_lock = self.access_hash_to_first_post_id.write().unwrap(); // TODO: Store in `.firebase` instead.
        let mut human_lock = self.human_readable_url.write().unwrap(); // TODO: Store in `.firebase` instead.
        let map = &mut *map_lock; // TODO: Don't use `map`, use `.firebase`.
        let login = &mut *login_lock; // TODO: Store in `.firebase` instead.
        let human = &mut *human_lock; // TODO: Store in `.firebase` instead.
        for maybe_post in posts {
            if let Some(mut post) = maybe_post {
                let key = post.id.clone();
                if post.human_readable_url == "" {
                    post.human_readable_url = to_url_part(&post.content);
                }
                if !login.contains_key(&post.access_hash) { // Update login info too.
                    login.insert(post.access_hash.clone(), key.clone()); // TODO: Store in `.firebase` instead.
                }
                if !human.contains_key(&post.human_readable_url) {
                    human.insert(post.human_readable_url.clone(), post.id.clone()); // TODO: Store in `.firebase` instead.
                }
                map.insert(key, post); // TODO: Don't use `map`, use `.firebase`. (Possibly, mutate many at once.)
            }
        }
    }

    /// Looks up the access hash in the database, to get the first post that was made by it.
    /// Useful for retrieving a post's author (another post).
    pub fn get_first_post(&self, access_hash: &str) -> Option<String> {
        let login_lock = self.access_hash_to_first_post_id.read().unwrap(); // TODO: Read from `.firebase` instead.
        let login = &*login_lock;
        return login.get(access_hash).map(|s| s.clone())
    }
    /// Authenticates a user's access token (username+password hashed), returning the first-post ID if there is such a user registered, else `None`.
    pub fn login(&self, user: &str) -> Option<String> {
        return self.get_first_post(&crate::posts_api::access_token_hash(user)) // TODO: Read from `.firebase` instead.
    }
    /// Converts a human-readable URL to the post ID, if present in the database.
    /// To get a post's URL, read `post.human_readable_url`: an empty string if not assigned.
    /// These URLs are auto-assigned, and will never collide with raw post IDs, nor with statically-served files (since these URLs are like `"2020_first_line_of_content"`).
    pub fn lookup_url(&self, url: &str) -> Option<String> {
        let mut human_lock = self.human_readable_url.write().unwrap(); // TODO: Read from `.firebase` instead.
        let human = &mut *human_lock;
        human.get(url.clone()).map(|s| s.clone())
    }
}



fn to_url_part(content: &str) -> String {
    let year = chrono::Utc::now().year().to_string();
    let simpler = match content.split_once('\n') {
        Some((line, _rest)) => line,
        None => content,
    };
    let simpler = simpler.replace(|c:char| !c.is_ascii_alphanumeric(), "_");
    let simpler = simpler.split('_').filter(|s| s.len() > 0).collect::<Vec<&str>>().join("_");
    let simpler = simpler.to_ascii_lowercase();
    year + "_" + if simpler.len() < 80 { &simpler } else { &simpler[..80] }
}