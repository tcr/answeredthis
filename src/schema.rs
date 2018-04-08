#![allow(unused_imports)]

table! {
    posts (id) {
        id -> Integer,
        title -> Text,
        asof -> Text,
        content -> Text,
        published -> Bool,
    }
}
