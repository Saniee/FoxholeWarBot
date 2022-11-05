import mongoose from 'mongoose';

const schema = new mongoose.Schema({
  guildID: String,
  shard: String,
  shardName: String,
});

const serverConfig: mongoose.Model<any> = mongoose.model(
  'serverConfig',
  schema
);

export = serverConfig;
