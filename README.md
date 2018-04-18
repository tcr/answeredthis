# answeredthis

dokku instructions:

```
dokku apps:create answeredthis
```

Then first push

```
dokku domains:add answeredthis answeredthis.com
dokku config:set --no-restart answeredthis DOKKU_LETSENCRYPT_EMAIL=<email>
dokku letsencrypt answeredthis

DATABASE_URL /storage/edit-timryan.sqlite3
```

```
cd frontend
npx webpack --watch ./src/index.tsx --mode development --output-filename='answeredthis.js' --output-path='../static'
```