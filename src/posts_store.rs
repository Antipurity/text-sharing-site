//! Stores posts.
//! This is a database-less thunk that simply stores posts in memory.
//!   TODO: Use Firebase, not this thunk.



use crate::posts_api::Post;

use std::sync::RwLock;
use std::collections::HashMap;



pub struct Database {
    posts: RwLock<HashMap<String, RwLock<Post>>>,
}



impl Database {
    /// Initializes the database connection.
    pub fn new() -> Database {
        Database{ posts: RwLock::new(HashMap::new()) }
    }
    /// Reads many posts from the database at once.
    pub fn read(&self, ids: Vec<&str>) -> Vec<Option<Post>> {
        let map = self.posts.read().unwrap();
        ids.iter().map(|id| {
            let maybe_post = (*map).get(id.clone()).map(|x| x.read().unwrap());
            maybe_post.map(|post| (*post).clone())
        }).collect()
    }
    /// Updates many posts in the database at once: read, process, write, as one atomic operation.
    pub fn update<F>(&self, ids: Vec<&str>, action: F)
    where F: FnOnce(Vec<Option<Post>>) -> Vec<Option<Post>> {
        let posts = self.read(ids);
        let posts = action(posts);
        let mut map_lock = self.posts.write().unwrap();
        let map = &mut *map_lock;
        for maybe_post in posts {
            match maybe_post {
                Some(post) => {
                    match map.get(&post.id) {
                        Some(post_locker) => {
                            // Actually, these per-post locks are kinda useless, since we're locking the whole map to read/write it anyway.
                            let mut lock = post_locker.write().unwrap();
                            (*lock).clone_from(&post);
                        },
                        None => {
                            map.insert(post.id.clone(), RwLock::new(post));
                        },
                    }
                },
                None => (),
            }
        }
    }
}