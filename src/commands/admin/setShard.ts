import { CommandInteraction, SlashCommandBuilder } from "discord.js";
import { CollectionName } from "../../../config.json";
import PocketBase from "pocketbase";

module.exports = {
  data: new SlashCommandBuilder()
    .setName("set-server")
    .setDescription(
      "Sets the server (Able, Baker, Charlie) the bot will get data from. Guild Side!"
    )
    .addStringOption((option) =>
      option
        .setName("server")
        .setDescription("Choose which server to get data from.")
        .setRequired(true)
        .addChoices(
          { name: "Able", value: "shard1" },
          { name: "Baker", value: "shard2" },
          { name: "Charlie", value: "shard3" }
        )
    ),
  async execute(interaction: CommandInteraction, pb: PocketBase) {
    var shardName = interaction.options.get("server")?.value;

    await interaction.deferReply({ ephemeral: true });

    const record = await pb
      .collection(CollectionName)
      .getFirstListItem(`guildId=${interaction.guildId}`)
      .catch((err) =>
        console.log(`No record for ${interaction.guild?.name} found!`)
      );

    if (record) {
      var data;
      if (shardName == "shard1") {
        data = {
          shard: "https://war-service-live.foxholeservices.com",
          shardName: "Able",
        };
      } else if (shardName == "shard2") {
        data = {
          shard: "https://war-service-live-2.foxholeservices.com",
          shardName: "Baker",
        };
      } else if (shardName == "shard3") {
        data = {
          shard: "https://war-service-live-3.foxholeservices.com",
          shardName: "Charlie",
        };
      }
      await pb
        .collection(CollectionName)
        .update(record.id, data)
        .then(() =>
          console.log(`Updated Record for ${interaction.guild?.name}!`)
        )
        .catch((err) => console.log(err));
      await interaction.editReply("Saved!");
    } else {
      var data;
      if (shardName == "shard1") {
        data = {
          guildId: `${interaction.guildId}`,
          shard: "https://war-service-live.foxholeservices.com",
          shardName: "Able",
        };
      } else if (shardName == "shard2") {
        data = {
          guildId: `${interaction.guildId}`,
          shard: "https://war-service-live-2.foxholeservices.com",
          shardName: "Baker",
        };
      } else if (shardName == "shard3") {
        data = {
          guildId: `${interaction.guildId}`,
          shard: "https://war-service-live-3.foxholeservices.com",
          shardName: "Charlie",
        };
      }
      await pb
        .collection(CollectionName)
        .create(data)
        .then(() =>
          console.log(`Created New Record for ${interaction.guild?.name}!`)
        )
        .catch((err) => console.log(err));
      await interaction.editReply("Saved!");
    }
    console.log(`Saved option ${shardName} for ${interaction.guild?.name}!`);
  },
};
