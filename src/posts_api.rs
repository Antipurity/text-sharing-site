use std::collections::HashMap;

mod hashing;
pub use hashing::access_token_hash;

use uuid::Uuid;
use serde_json::json;
use handlebars::JsonValue;



/// Creates a string that has practically no chance of being the same as another string.
fn new_uuid() -> String {
    Uuid::new_v4().to_string()
}



/// A post: a published piece of information, communicated through Markdown.
/// 
/// For example, a user is just another kind of post (with a new access_hash).
/// 
/// ```
/// assert_eq!(5*5, 25)
/// ```
#[derive(Clone)]
pub struct Post {
    pub id: String,
    access_hash: String, // Username&password are concatenated & hashed to produce this.
    //   (No collisions this way. Especially if an access-file is used instead.)
    //   (Gates write access: creating posts, editing and 'deleting' them, and rewarding any posts.)
    //     (Password is copied into posts, so can't change it.)
    content: String, // Intended to be Markdown, with the first line displayed as the title.
    reward: i64, // Less than -10 should get deleted.
    parent_id: String,
    children_rights: Vec<String>, // An empty string, usually (must be empty to disallow comments).
    children_ids: Vec<String>,
    rewarded_sum: i8,
    rewarded_posts: HashMap<String, i8>, // -100 (only own posts), -1, 1.
    //   (Only non-empty for initial access_hash posts, meaning, user accounts.)
    //   (Sum of non -100 rewards should be -10..=10, for balance.)
    created_post_ids: Vec<String>,
}

impl Post {
    /// Creates a new, top-level, open-to-comments, post.
    /// Exists because we have to root the post tree in *something*.
    pub fn new_public(id: Option<String>, content: String) -> Post {
        Post {
            id: id.unwrap_or_else(|| new_uuid()),
            access_hash: "".to_string(), // No user can edit it (except for our own functions).
            content,
            reward: 0i64,
            parent_id: "".to_string(), // No parent.
            children_rights: vec!["".to_string()], // Open to all comments.
            children_ids: Vec::new(),
            rewarded_sum: 0i8,
            rewarded_posts: HashMap::new(),
            created_post_ids: Vec::new(),
        }
    }
    /// Adds a new child-post to a parent-post.
    /// Returns (user_first_post, parent, Option<child>).
    pub fn new(mut parent: Post, mut user_first_post: Post, content: String, children_rights: Vec<String>) -> (Post, Post, Option<Post>) {
        let hash = &user_first_post.access_hash;
        if parent.children_rights.iter().any(|s| s == "" || s == hash) {
            let id = new_uuid();
            user_first_post.created_post_ids.push(id.clone());
            parent.children_ids.push(id.clone());
            let parent_id = parent.id.clone();
            let access_hash = hash.to_string();
            std::mem::drop(hash);
            (
                user_first_post,
                parent,
                Some(Post {
                    id,
                    access_hash,
                    content,
                    reward: 0i64,
                    parent_id,
                    children_rights,
                    children_ids: Vec::new(),
                    rewarded_sum: 0i8,
                    rewarded_posts: HashMap::new(),
                    created_post_ids: Vec::new(),
                })
            )
        } else {
            (user_first_post, parent, None)
        }
    }
    /// Changes a post's content and its openness-to-comments status.
    pub fn edit(self: Post, user: String, content: String, children_rights: Vec<String>) -> Option<Post> {
        if access_token_hash(&user) == self.access_hash {
            Some(Post {
                content,
                children_rights,
                ..self
            })
        } else {
            None
        }
    }
    /// Gives reward to a post, from a user: -100|-1|1.
    /// Only succeeds if the user has given up to ±10 of ±1 rewards, to force normalization.
    /// Returns (user_first_post, Option<rewarded_post>).
    pub fn reward(self: Post, mut user_first_post: Post, amount: i8) -> (Post, Option<Post>) {
        if &self.access_hash == &user_first_post.access_hash {
            return (user_first_post, None)
        }
        if amount != -100 && amount != -1 && amount != 1 {
            return (user_first_post, None)
        };
        if amount == -100 && self.access_hash != user_first_post.access_hash {
            return (user_first_post, None)
        };
        if amount != -100 {
            if user_first_post.rewarded_sum < -10 || user_first_post.rewarded_sum > 10 {
                return (user_first_post, None)
            }
            user_first_post.rewarded_sum += amount;
        };
        *user_first_post.rewarded_posts.entry(self.id.clone()).or_insert(0i8) = amount;
        return (user_first_post, Some(Post{
            reward: self.reward + (amount as i64),
            ..self
        }))
    }

    /// Returns `{ content, post_reward, user_reward, parent_id, children_rights }` as a JSON object. (`.to_string()` will convert it to a JSON string.)
    /// `content` and `parent_id` are strings,  rewards are integers, `children_rights` is an array of strings.
    pub fn to_json(self: &Post, user: Option<&Post>) -> JsonValue {
        json!({
            "content": self.content,
            "post_reward": self.reward,
            "user_reward": match user {
                Some(u) => u.rewarded_posts.get(&self.id).map_or(0i8, |r| *r),
                None => 0i8,
             },
            "parent_id": self.parent_id,
            "children_rights": self.children_rights,
            "children": self.children_ids.len(),
            "access_hash": self.access_hash,
        })
    }

    /// Gets the specified child-post IDs of a post, most-recent first.
    /// (Currently not optimized, because there's no need.)
    pub fn get_children_newest_first(&self, start:usize, end:usize) -> Result<Vec<String>, ()> {
        if start >= end {
            let iter = self.children_ids.iter().rev().skip(start).take(end-start);
            Ok(iter.map(|s| s.clone()).collect())
        } else {
            Err(())
        }
    }

    /// Gets the specified rewarded-post IDs of a user's first post, most-recent first.
    /// (Currently not optimized, because there's no need.)
    pub fn get_rewarded_posts(&self, start:usize, end:usize) -> HashMap<&String, &i8> {
        let mut v: Vec<_> = self.rewarded_posts.iter().collect(); // Key-value pairs.
        v.reverse();
        v[start..end].iter().map(|p| *p).collect()
    }

    /// Gets the specified created-post IDs of a user's first post, most-recent first.
    /// (Currently not optimized, because there's no need.)
    pub fn get_created_posts(&self, start:usize, end:usize) -> Result<Vec<String>, ()> {
        if start >= end {
            let iter = self.created_post_ids.iter().rev().skip(start).take(end-start);
            Ok(iter.map(|s| s.clone()).collect())
        } else {
            Err(())
        }
    }
}