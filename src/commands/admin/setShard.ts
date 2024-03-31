import { CommandInteraction, SlashCommandBuilder } from "discord.js";
import { CollectionName } from "../../../config.json";
import PocketBase from "pocketbase";
import axios from "axios";

async function checkStatusCode(shardName: any) {
  const servers = [
    { name: "Able", url: "https://war-service-live.foxholeservices.com" },
    { name: "Baker", url: "https://war-service-live-2.foxholeservices.com" },
    { name: "Charlie", url: "https://war-service-live-3.foxholeservices.com" },
  ];

  if (shardName === "shard1") {
    const response = axios.get(`${servers[0].url}/api/worldconquest/maps`, {
      validateStatus: function (status) {
        return status < 600;
      },
    });
    return (await response).status;
  } else if (shardName === "shard2") {
    const response = axios.get(`${servers[1].url}/api/worldconquest/maps`, {
      validateStatus: function (status) {
        return status < 600;
      },
    });
    return (await response).status;
  } else if (shardName === "shard3") {
    const response = axios.get(`${servers[2].url}/api/worldconquest/maps`, {
      validateStatus: function (status) {
        return status < 600;
      },
    });
    return (await response).status;
  }
}

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
    const record = await pb
      .collection(CollectionName)
      .getFirstListItem(`guildId=${interaction.guildId}`)
      .catch((err) =>
        console.log(
          `No record for ${interaction.guild?.name} found! (/set-server)`
        )
      );

    var shardName = interaction.options.get("server")?.value;

    if (record) {
      await interaction.deferReply({ ephemeral: !record.showCommandOutput });
      const status = await checkStatusCode(shardName);
      if (status === 503) {
        await interaction.editReply(
          "Server is not Online/Temporarily Unavailable. Choose a different one."
        );
        return;
      }
      if (status === 504) {
        await interaction.editReply(
          "Server is not Online/Temporarily Unavailable. Choose a different one. [Connection Timed out.]"
        );
        return;
      }
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
      await interaction.deferReply({ ephemeral: true });
      var data;
      if (shardName == "shard1") {
        data = {
          guildId: `${interaction.guildId}`,
          shard: "https://war-service-live.foxholeservices.com",
          shardName: "Able",
          showCommandOutput: false,
        };
      } else if (shardName == "shard2") {
        data = {
          guildId: `${interaction.guildId}`,
          shard: "https://war-service-live-2.foxholeservices.com",
          shardName: "Baker",
          showCommandOutput: false,
        };
      } else if (shardName == "shard3") {
        data = {
          guildId: `${interaction.guildId}`,
          shard: "https://war-service-live-3.foxholeservices.com",
          shardName: "Charlie",
          showCommandOutput: false,
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
