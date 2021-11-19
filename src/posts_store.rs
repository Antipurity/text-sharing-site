//! Stores posts.
//! This is a database-less thunk that simply stores posts in memory.
//!   TODO: Use Firebase, not this thunk.
//!   TODO: Collections that we want: posts; access_hash_to_first_post_id; human_readable_url.
//!   TODO: Finish changing this file, and `posts_api` too.
//!   TODO: Test it, and make it work.



use crate::posts_api::Post;

use std::sync::Arc;
use std::sync::Mutex;

use chrono::Datelike;
use firebase_rs::Firebase;
use serde_json::{from_str, to_string};



pub struct Database {
    pub firebase: Firebase, // I guess this doesn't need our synchronization.
}



impl Database {
    /// Initializes the database connection.
    pub fn new(fb: Firebase) -> Database {
        Database{
            firebase: fb,
        }
    }
    /// Reads many posts from the database at once.
    pub fn read(&self, ids: Vec<&str>) -> Vec<Option<Post>> {
        // `firebase_rs`'s `.get_async` API is really dumb. It's forcing Arc and Mutex on us.
        let fb = &self.firebase;
        let mut values: Vec<Arc<Mutex<Option<Post>>>> = vec![];
        let mut handles: Vec<Option<std::thread::JoinHandle<()>>> = vec![];
        for (i, id) in ids.iter().enumerate() {
            values.push(Arc::new(Mutex::new(None)));
            let item = values[i].clone();
            let maybe_node = fb.at(&("posts/".to_owned() + id + "/data")).ok();
            handles.push(maybe_node.map(|node| node.get_async(move |res| {
                let maybe_r = res.ok();
                if let Some(ref r) = maybe_r {
                    println!("{}", r.body); // TODO: We do fetch it. So it's a deserialization error. Need to remove maps.
                }
                let maybe_r = maybe_r.map(|r| from_str(&r.body).ok()).flatten();
                *item.lock().unwrap() = maybe_r;
            })));
        }
        for maybe_handle in handles { maybe_handle.map(|h| h.join().unwrap()); }
        values.drain(..).map(|mutex_ptr| {
            match Arc::try_unwrap(mutex_ptr).ok() {
                Some(mutex) => mutex.into_inner().unwrap(),
                None => None,
            }
        }).collect()
    }
    /// Updates many posts in the database at once: read, process, write, as one "atomic" operation.
    /// 
    /// (Well, "atomic" is a word too strong for this: it was a nice thought, but firebase-rs has never heard of atomicity, and we don't care enough to implement transactions ourselves (https://stackoverflow.com/questions/23041800/firebase-transactions-via-rest-api).)
    /// (It first reads all, then writes all, each of these being atomic.)
    /// (And, `posts_api` reads/updates `children_ids`, `rewarded_posts`, `created_post_ids` directly, with no regard for atomicity.)
    pub fn update<F>(&self, ids: Vec<&str>, action: F)
    where F: FnOnce(Vec<Option<Post>>) -> Vec<Option<Post>> {
        let posts = self.read(ids);
        let posts = action(posts);
        let fb = &self.firebase;
        let mut handles: Vec<std::thread::JoinHandle<()>> = vec![];
        for maybe_post in posts {
            if let Some(mut post) = maybe_post {
                if post.human_readable_url == "" {
                    post.human_readable_url = to_url_part(&post.content);
                }
                fb.at(&("access_hash/".to_owned() + &post.access_hash)).ok().map(|node| {
                    let b = to_string(&post.id).ok();
                    b.map(|body| handles.push(node.update_async(body, |_| ())));
                });
                fb.at(&("human_readable_url/".to_owned() + &post.human_readable_url)).ok().map(|node| {
                    let b = to_string(&post.id).ok();
                    b.map(|body| handles.push(node.update_async(body, |_| ())));
                });
                fb.at(&("posts/".to_owned() + &post.id + "/data")).ok().map(|node| {
                    let b = to_string(&post).ok();
                    b.map(|body| handles.push(node.set_async(body, |_| ())));
                });
            }
        }
        for handle in handles { handle.join().unwrap(); }
    }

    /// Looks up the access hash in the database, to get the first post that was made by it.
    /// Useful for retrieving a post's author (another post).
    pub fn get_first_post(&self, access_hash: &str) -> Option<String> {
        let fb = &self.firebase;
        fb.at(&("access_hash/".to_owned() + access_hash)).ok().map(|node| {
            node.get().ok().map(|r| from_str(&r.body).ok()).flatten()
        }).flatten()
    }
    /// Authenticates a user's access token (username+password hashed), returning the first-post ID if there is such a user registered, else `None`.
    pub fn login(&self, user: &str) -> Option<String> {
        return self.get_first_post(&crate::posts_api::access_token_hash(user))
    }
    /// Converts a human-readable URL to the post ID, if present in the database.
    /// To get a post's URL, read `post.human_readable_url`: an empty string if not assigned.
    /// These URLs are auto-assigned, and will never collide with raw post IDs, nor with statically-served files (since these URLs are like `"2020_first_line_of_content"`).
    pub fn lookup_url(&self, url: &str) -> Option<String> {
        let fb = &self.firebase;
        fb.at(&("human_readable_url/".to_owned() + url)).ok().map(|node| {
            node.get().ok().map(|r| from_str(&r.body).ok()).flatten()
        }).flatten()
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