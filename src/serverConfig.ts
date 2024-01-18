import mongoose from "mongoose";

const schema = new mongoose.Schema({
  guildID: String,
  shard: String,
  shardName: String,
});

const serverConfig = mongoose.model("serverConfig", schema);

export default serverConfig;
