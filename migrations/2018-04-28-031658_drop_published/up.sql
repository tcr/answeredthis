CREATE TEMPORARY TABLE posts_backup(id,title,asof,content);
INSERT INTO posts_backup SELECT id,title,asof,content FROM posts;
DROP TABLE posts;
CREATE TABLE posts(id,title,asof,content);
INSERT INTO posts SELECT id,title,asof,content FROM posts_backup;
DROP TABLE posts_backup;
