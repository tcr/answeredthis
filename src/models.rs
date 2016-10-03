use super::schema::posts;

#[derive(Queryable, Debug)]
pub struct Post {
    pub id: i32,
    pub title: String,
    pub asof: String,
    pub content: String,
    pub published: bool,
}

#[insertable_into(posts)]
pub struct NewPost<'a> {
    pub title: &'a str,
    pub asof: &'a str,
    pub content: &'a str,
}
