use std::collections::HashMap;

use uuid::Uuid;
use serde_json::json;

mod hashing;
use hashing::access_token_hash;



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
struct Post {
    id: String,
    access_hash: String, // Username&password are concatenated & hashed to produce this.
    //   (No collisions this way. Especially if an access-file is used instead.)
    //   (Gates write access: creating posts, editing and 'deleting' them, and rewarding any posts.)
    //     (Password is copied into posts, so can't change it.)
    content: String, // Intended to be Markdown, with the first line displayed as the title.
    reward: i64, // Less than -10 should get deleted.
    parent_id: String,
    children_rights: Vec<String>, // An empty string, usually (must be empty to disallow comments).
    children_ids: Vec<String>,
    rewarded_posts: HashMap<String, i8>, // -100 (only own posts), -1, 1.
    //   (Only non-empty for initial access_hash posts, meaning, user accounts.)
    //   (Sum of non -100 rewards should be -10..=10, for balance.)
    created_post_ids: Vec<String>,
}

impl Post {
    /// Creates a new, top-level, open-to-comments, post.
    /// Exists because we have to root the post tree in *something*.
    pub fn new_public(content: String) -> Post {
        Post {
            id: new_uuid(),
            access_hash: "".to_string(), // No user can edit it (except for our own functions).
            content,
            reward: 0i64,
            parent_id: "".to_string(), // No parent.
            children_rights: vec!["".to_string()], // Open to all comments.
            children_ids: Vec::new(),
            rewarded_posts: HashMap::new(),
            created_post_ids: Vec::new(),
        }
    }
    /// Adds a new child-post to a parent-post.
    /// Returns (user, parent, Option<child>).
    pub fn new(mut parent: Post, mut user: Post, content: String, children_rights: Vec<String>) -> (Post, Post, Option<Post>) {
        let hash = &user.access_hash;
        if parent.children_rights.iter().any(|s| s == "" || s == hash) {
            let id = new_uuid();
            user.created_post_ids.push(id.clone());
            parent.children_ids.push(id.clone());
            let parent_id = parent.id.clone();
            let access_hash = hash.to_string();
            std::mem::drop(hash);
            (
                user,
                parent,
                Some(Post {
                    id,
                    access_hash,
                    content,
                    reward: 0i64,
                    parent_id,
                    children_rights,
                    children_ids: Vec::new(),
                    rewarded_posts: HashMap::new(),
                    created_post_ids: Vec::new(),
                })
            )
        } else {
            (user, parent, None)
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
    pub fn reward(self: Post, user: String, amount: i8) -> Option<Post> {
        let hash = access_token_hash(&user);
        if amount != -100 && amount != -1 && amount != 1 {
            return None
        };
        if amount == -100 && self.access_hash != user {
            return None
        };
        // TODO: ???.rewarded_posts[&self.id] = amount
        return Some(Post{
            reward: self.reward + (amount as i64),
            ..self
        })
    }
    /// Returns `{ content, post_reward, user_reward, parent_id, children_rights }` as a JSON string.
    /// `content` and `parent_id` are strings,  rewards are integers, `children_rights` is an array of strings.
    pub fn to_json(self: &Post, user: &Post) -> String {
        // TODO: Also, accept the user, and return how much said user has rewarded this post.
        json!({
            "content": self.content,
            "post_reward": self.reward,
            "user_reward": user.rewarded_posts[&self.id],
            "parent_id": self.parent_id,
            "children_rights": self.children_rights,
        }).to_string()
    }

    // TODO: Start..end slices:
    //   TODO: All children of a post.
    //     TODO: Sorted by date.
    //     TODO: Sorted by reward.
    //   TODO: All rewarded-post-IDs of a user (access_token).
    //   TODO: All created-post-IDs of a user (access_token).
    //   …Doesn't this mean that children_ids and rewarded_posts should not be stored so directly on Post…

    // TODO: ...Also, string->Post conversion, for easy DB look up...
    //   Do we really want that, though... Not like it's very useful if DB lookup is async.
}