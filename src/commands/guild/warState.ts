import {
  SlashCommandBuilder,
  CommandInteraction,
  EmbedBuilder,
  Colors,
  AttachmentBuilder,
  AutocompleteInteraction,
} from "discord.js";
import axios from "axios";
import { CollectionName } from "../../../config.json";
import PocketBase, { RecordModel } from "pocketbase";

module.exports = {
  data: new SlashCommandBuilder()
    .setName("war-state")
    .setDescription(
      "Gets the global state of the war. This command will default to not showing."
    )
    .addBooleanOption((option) =>
      option.setName("show").setDescription("Show the response?")
    ),
  async execute(interaction: CommandInteraction, pb: PocketBase) {
    var show = !interaction.options.get("show")?.value;

    const record = await pb
      .collection(CollectionName)
      .getFirstListItem(`guildId=${interaction.guildId}`)
      .catch((err) =>
        console.log(`No record for ${interaction.guild?.name} found!`)
      );

    await interaction.deferReply({ ephemeral: show });

    if (record) {
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
    }
  },
};
