import {
  SlashCommandBuilder,
  CommandInteraction,
  EmbedBuilder,
  Colors,
} from "discord.js";
import axios from "axios";
import { CollectionName } from "../../../config.json";
import PocketBase from "pocketbase";

module.exports = {
  data: new SlashCommandBuilder()
    .setName("war-state")
    .setDescription(
      "Gets the global state of the war. This command will default to not showing."
    ),
  async execute(interaction: CommandInteraction, pb: PocketBase) {
    const record = await pb
      .collection(CollectionName)
      .getFirstListItem(`guildId=${interaction.guildId}`)
      .catch((err) =>
        console.log(
          `No record for ${interaction.guild?.name} found! (/war-state)`
        )
      );

    if (record) {
      await interaction.deferReply({ ephemeral: !record.showCommandOutput });
      const warStateURL = `${record.shard}/api/worldconquest/war`;

      const response = axios.get(warStateURL, {
        validateStatus: function (status) {
          return status < 600;
        },
      });

      const statusCode = (await response).status;

      switch (statusCode) {
        case 503:
          await interaction.editReply({
            content:
              "Server not Online/Temporarily Unavailable. Please Change to Shard1/Shard2",
          });
          break;
        case 404:
          await interaction.editReply({
            content: "No War State for that HEX found!",
          });
          break;
        case 200:
          console.log(
            `Api request for: WarState, Response Code: ${
              (await response).status
            }`
          );
          const data = (await response).data;
          let startTime = new Date(data.conquestStartTime);
          const warState = new EmbedBuilder()
            .setColor(Colors.Red)
            .addFields(
              { name: "Shard/Server", value: `${record.shardName}` },
              { name: "War Number", value: `${data.warNumber}` },
              { name: "Winner", value: `${data.winner}` },
              { name: "Conquest Start Time", value: `${startTime}` },
              {
                name: "Required Victory Towns",
                value: `${data.requiredVictoryTowns}`,
              }
            )
            .setFooter({ text: "Requested at:" })
            .setTimestamp(new Date());

          await interaction.editReply({ embeds: [warState] });
          break;
      }
    } else {
      await interaction.deferReply({ ephemeral: true });
      interaction.editReply({
        content:
          "Server setting missing, please run the command `/set-server` to fix this issue!",
      });
    }
  },
};
