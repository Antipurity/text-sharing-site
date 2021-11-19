use std::collections::HashMap;

mod hashing;
pub use hashing::access_token_hash;

use uuid::Uuid;
use serde_json::json;
use handlebars::JsonValue;



/// Creates a string that has practically no chance of being the same as another string.
pub fn new_uuid() -> String {
    Uuid::new_v4().to_string()
}



/// Who can create sub-posts in the parent post.
#[derive(Clone)]
pub enum CanPost {
    None,
    Itself,
    All,
}



// TODO: A function for atomically updating a Firebase counter (read, then as long as write (update(response)) fails, repeat read+write).
//   (Each counter has to have an entry in the DB, checking that each update makes sense.)



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
    pub access_hash: String, // Username&password are concatenated & hashed to produce this.
    //   (No collisions this way. Especially if an access-file is used instead.)
    //   (Gates write access: creating posts, editing and 'deleting' them, and rewarding any posts.)
    //     (Password is copied into posts, so can't change it.)
    pub human_readable_url: String, // A human-readable name, such as "2020_first_line".
    pub content: String, // Intended to be Markdown, with the first line displayed as the title.
    reward: i64, // Less than -10 should get deleted.
    parent_id: String,
    children_rights: CanPost,
    children_ids: Vec<String>, // TODO: Don't have this.
    rewarded_sum: i8,
    rewarded_posts: HashMap<String, i8>, // -100 (only own posts), -1, 1. // TODO: Don't have this.
    //   (Only non-empty for initial access_hash posts, meaning, user accounts.)
    //   (Sum of non -100 rewards should be -10..=10, for balance.)
    created_post_ids: Vec<String>, // TODO: Don't have this.
}

impl Post {
    /// Creates a new, top-level, open-to-comments, post.
    /// Exists because we have to root the post tree in *something*.
    pub fn new_public(id: Option<String>, content: String) -> Post {
        let id = id.unwrap_or_else(|| new_uuid());
        Post {
            id: id.clone(),
            access_hash: "".to_string(), // No user can edit it (except for our own functions).
            human_readable_url: "".to_string(),
            content,
            reward: 0i64,
            parent_id: id,
            children_rights: CanPost::All,
            children_ids: Vec::new(), // TODO: Don't do this.
            rewarded_sum: 0i8,
            rewarded_posts: HashMap::new(), // TODO: Don't do this.
            created_post_ids: Vec::new(), // TODO: Don't do this.
        }
    }
    /// Adds a new child-post to a parent-post.
    /// `access_hash` must be `crate::posts_api::access_token_hash(user)`.
    /// `user_first_post` must be `posts_store::Database::login(self, user).map(|id| database.read(vec![id]).pop().unwrap())`.
    /// Returns (parent, Option<user_first_post>, Option<child>).
    pub fn new(mut parent: Post, access_hash: &str, user_first_post: Option<Post>, content: String, children_rights: CanPost) -> (Post, Option<Post>, Option<Post>) {
        let rights = &parent.children_rights;
        let (same, mut user_first_post) = match user_first_post {
            Some(ref post) => if post.id == parent.id { (true, None) } else { (false, user_first_post) },
            None => (false, user_first_post),
        };
        if matches!(rights, CanPost::All) || matches!(rights, CanPost::Itself) && &parent.access_hash == access_hash {
            // TODO: Also accept `firebase`, and do `fb.at("posts_created_post_ids").unwrap().at(parent.id).unwrap().push(id).unwrap()`, except, handling errors, and possibly `.push_async(…, |_| "ignore response")`.
            //   And at "posts_children_ids".
            let id = new_uuid();
            if let Some(ref mut post) = user_first_post {
                post.created_post_ids.push(id.clone())
            };
            if same {
                parent.created_post_ids.push(id.clone())
            };
            parent.children_ids.push(id.clone());
            let parent_id = parent.id.clone();
            (
                parent,
                if same {None} else {user_first_post},
                Some(Post {
                    id,
                    access_hash: access_hash.to_string(),
                    human_readable_url: "".to_string(),
                    content,
                    reward: 0i64,
                    parent_id,
                    children_rights,
                    children_ids: Vec::new(), // TODO: Don't do this.
                    rewarded_sum: 0i8,
                    rewarded_posts: HashMap::new(), // TODO: Don't do this.
                    created_post_ids: Vec::new(), // TODO: Don't do this.
                })
            )
        } else {
            (parent, if same {None} else {user_first_post}, None)
        }
    }
    /// Changes a post's content and its openness-to-comments status.
    pub fn edit(self: Post, user: &str, content: String, children_rights: CanPost) -> Option<Post> {
        if access_token_hash(user) == self.access_hash && self.access_hash != "" {
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
        if amount != -100 && amount != -1 && amount != 0 && amount != 1 {
            return (user_first_post, None)
        };
        if amount == -100 && (self.access_hash != user_first_post.access_hash || self.access_hash == "") {
            return (user_first_post, None)
        };
        if amount != -100 {
            let will_be = user_first_post.rewarded_sum + amount;
            if will_be < -10 || will_be > 10 {
                return (user_first_post, None)
            }
            user_first_post.rewarded_sum += amount;
        };
        let map = &mut user_first_post.rewarded_posts; // TODO: Update a Firebase per-user counter, not a HashMap.
        let delta = if amount != 0 {
            let old = map.insert(self.id.clone(), amount).unwrap_or(0i8); // TODO: Firebase.
            amount - old
        } else {
            match map.remove(&self.id) { // TODO: Firebase.
                Some(old) => -old,
                None => 0i8,
            }
        };
        if self.id != user_first_post.id {
            (user_first_post, Some(Post{
                reward: self.reward + (delta as i64),
                ..self
            }))
        } else {
            (Post{
                reward: self.reward + (delta as i64),
                ..user_first_post
            }, None)
        }
    }

    /// Returns `{ content, post_reward, user_reward, parent_id, children_rights, children, access_hash, human_readable_url, logged_in }` as a JSON object. (`.to_string()` will convert it to a JSON string.)
    /// `content` and `parent_id` and `human_readable_url` are strings, rewards are integers, `children_rights` is 'none'|'itself'|'all', `children` is how many children this post has, `access_hash` is what the owner's access token must hash to, `logged_in` is a boolean.
    pub fn to_json(self: &Post, user: Option<&Post>) -> JsonValue {
        json!({
            "id": self.id,
            "content": self.content,
            "post_reward": self.reward,
            "user_reward": match user {
                Some(u) => u.rewarded_posts.get(&self.id).map_or(0i8, |r| *r), // TODO: Read from Firebase.
                None => 0i8,
            },
            "parent_id": self.parent_id,
            "children_rights": self.children_rights.to_string(),
            "children": self.children_ids.len(), // TODO: Read this from Firebase. (In fact, may have to store the array length separately, because Firebase for SOME reason doesn't support querying array length.)
            "access_hash": self.access_hash,
            "human_readable_url": "/post/".to_owned() + if &self.human_readable_url == "" {
                &self.id
            }  else {
                &self.human_readable_url
            },
            "logged_in": user.is_some(),
        })
    }

    /// Gets the specified child-post IDs of a post, most-recent first.
    /// (Currently not optimized, because there's no need.)
    pub fn get_children_newest_first(&self, start:usize, end:usize) -> Result<Vec<String>, ()> {
        let start = std::cmp::min(start, self.children_ids.len()); // TODO: Read this from Firebase.
        let end = std::cmp::min(end, self.children_ids.len());
        if start <= end {
            let iter = self.children_ids.iter().rev().skip(start).take(end-start); // TODO: Read from Firebase, via `.at("posts_children_ids").unwrap().with_params().start_at(start).limit_to_first(end-start).get().unwrap()` but with error-handling. (Maybe Post should have the date, so that we can `.order_by("reverse_date_created").`)
            Ok(iter.map(|s| s.clone()).collect())
        } else {
            Err(())
        }
    }

    /// Gets the specified rewarded-post IDs of a user's first post, in an arbitrary order.
    /// (Currently not optimized, because there's no need.)
    pub fn get_rewarded_posts(&self, start:usize, end:usize) -> Result<Vec<String>, ()> {
        let start = std::cmp::min(start, self.rewarded_posts.len()); // TODO: Read from Firebase.
        let end = std::cmp::min(end, self.rewarded_posts.len());
        if start <= end {
            let iter = self.rewarded_posts.keys().skip(start).take(end-start); // TODO: Read from Firebase, via `.at("posts_rewarded_posts").unwrap().with_params().start_at(start).limit_to_first(end-start).get().unwrap()` but with error-handling.
            Ok(iter.map(|s| s.clone()).collect())
        } else {
            Err(())
        }
    }
    pub fn get_rewarded_posts_length(&self) -> usize {
        self.rewarded_posts.len() // TODO: Read from a Firebase counter.
    }

    /// Gets the specified created-post IDs of a user's first post, most-recent first.
    /// (Currently not optimized, because there's no need.)
    pub fn get_created_posts(&self, start:usize, end:usize) -> Result<Vec<String>, ()> {
        let start = std::cmp::min(start, self.created_post_ids.len()); // TODO: Read from Firebase (need to store a separate counter for this).
        let end = std::cmp::min(end, self.created_post_ids.len());
        if start <= end {
            let iter = self.created_post_ids.iter().rev().skip(start).take(end-start); // TODO: Read from Firebase, via `.at("posts_created_post_ids").unwrap().with_params().start_at(start).limit_to_first(end-start).get().unwrap()` but with error-handling. (Maybe Post should have the negated reward, so that we can `.order_by("reverse_reward").`)
            Ok(iter.map(|s| s.clone()).collect())
        } else {
            Err(())
        }
    }
    pub fn get_created_posts_length(&self) -> usize {
        self.created_post_ids.len() // TODO: Read from Firebase.
    }
}



impl ToString for CanPost {
    fn to_string(&self) -> String {
        match self {
            Self::None => "none",
            Self::Itself => "itself",
            Self::All => "all",
        }.to_string()
    }
}
impl core::str::FromStr for CanPost {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(Self::None),
            "itself" => Ok(Self::Itself),
            "all" => Ok(Self::All),
            _ => Err(()),
        }
    }
}