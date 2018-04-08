# answeredthis

dokku instructions:

```
dokku apps:create answeredthis
```

Then first push

```
dokku domains:add answeredthis answeredthis.com
dokku letsencrypt answeredthis
dokku config:set --no-restart answeredthis DOKKU_LETSENCRYPT_EMAIL=<email>

DATABASE_URL /storage/edit-timryan.sqlite3
```