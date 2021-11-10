//! Stores posts.
//! This is a database-less thunk that simply stores posts in memory.



use crate::posts_api::Post;

use std::sync::Once;
use std::sync::RwLock;
use std::collections::HashMap;
static data: Once = Once::new(); // TODO: ...No, I don't think this is any good...
// TODO: What's the correct solution to create a singleton?
//   Typed RwLock<HashMap<String, Post>>…

fn get_data() -> &HashMap<String, Post> {
    data.call_once() // TODO: Wait, no, why doesn't this return any results? HOW DO WE STORE THE CREATED HASH MAP
}



/// Reads one post from the database.
pub fn read(id: &String) -> Option<Post> {
    data.get(id).map(|x| (*x).clone())
}

// TODO: Set: `pub fn update([…ids], |[…posts]| updated_posts)`.

// TODO: A thunk, using global maps instead of a database.