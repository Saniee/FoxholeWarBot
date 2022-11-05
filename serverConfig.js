const mongoose = require('mongoose')

const schema = new mongoose.Schema({
    guildID: String,
    shard: String,
    shardName: String,
});

const serverConfig = mongoose.model(
    'serverConfig',
    schema
);

module.exports = serverConfig;