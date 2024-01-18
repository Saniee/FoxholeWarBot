import {
  SlashCommandBuilder,
  CommandInteraction,
  EmbedBuilder,
  Colors,
  AttachmentBuilder,
  AutocompleteInteraction,
} from "discord.js";
import axios from "axios";
import fs from "node:fs";
import path from "node:path";
import { createCanvas, loadImage } from "canvas";
import { CollectionName } from "../../../config.json";
import PocketBase, { RecordModel } from "pocketbase";

module.exports = {
  data: new SlashCommandBuilder()
    .setName("war-report")
    .setDescription("Responds with information about a Hex/Map Chunk.")
    .addStringOption((option) =>
      option
        .setName("map-name")
        .setDescription("Name of the Hex you want displayed.")
        .setRequired(true)
        .setAutocomplete(true)
    ),
  async autocomplete(interaction: AutocompleteInteraction, pb: PocketBase) {
    const record = await pb
      .collection(CollectionName)
      .getFirstListItem(`guildId=${interaction.guildId}`)
      .catch((err) =>
        console.log(`No record for ${interaction.guild?.name} found!`)
      );

    if (record) {
      const fileData = fs.readFileSync(
        path.resolve(
          __dirname,
          `../../cache/mapChoices/${record.shardName}Hexes.json`
        ),
        { encoding: "utf-8", flag: "r" }
      );

      var mapData = await JSON.parse(fileData);
      const focusedValue = interaction.options.getFocused();
      const choices = mapData;
      const filtered = choices.filter((choice) =>
        choice.startsWith(focusedValue)
      );

      let options;
      if (filtered.length > 25) {
        options = filtered.slice(0, 25);
      } else {
        options = filtered;
      }

      await interaction.respond(
        options.map((choice) => ({
          name: choice.replace("Hex", ""),
          value: choice,
        }))
      );
    } else {
      const focusedValue = interaction.options.getFocused();
      const choices = [
        "There was an error. First try to run /set-server.",
        "If that doesn't work go to the support server.",
      ];
      const filtered = choices.filter((choice) =>
        choice.startsWith(focusedValue)
      );

      let options;
      options = filtered;

      await interaction.respond(
        options.map((choice) => ({
          name: choice.replace("Hex", ""),
          value: choice,
        }))
      );
    }
  },
  async execute(interaction: CommandInteraction, pb: PocketBase) {
    var mapName = interaction.options.get("map-name")?.value;

    const record = await pb
      .collection(CollectionName)
      .getFirstListItem(`guildId=${interaction.guildId}`)
      .catch((err) =>
        console.log(`No record for ${interaction.guild?.name} found!`)
      );

    await interaction.deferReply({ ephemeral: true });

    if (record) {
      fs.readFile(
        path.resolve(
          __dirname,
          `../../cache/warReport/${mapName}${record.shardName}.json`
        ),
        "utf-8",
        async (err, fileData) => {
          if (fileData) {
            const cache = JSON.parse(fileData);
            await apiRequest(cache, record, interaction, mapName);
          } else if (!fileData) {
            const cache = {
              version: "0",
            };
            await apiRequest(cache, record, interaction, mapName);
          } else {
            console.log(err);
            interaction.editReply("An error has occured!");
          }
        }
      );
    } else {
      interaction.editReply({
        content:
          "Shard setting missing, please run the command `War!setShard {shard1 | shard2}` to fix this issue!",
      });
    }
  },
};

async function apiRequest(cache, record, interaction, mapName) {
  const warReportURL = `${record.shard}/api/worldconquest/warReport/${mapName}`;

  try {
    const response = axios.get(warReportURL, {
      validateStatus: function (status) {
        return status < 600;
      },
      headers: { "If-None-Match": `"${cache.version}"` },
    });

    const statusCode = (await response).status;

    switch (statusCode) {
      case 503:
        interaction.editReply({
          content:
            "Server not Online/Temporarily Unavailable. Please Change to Shard1/Shard2",
        });
        break;
      case 404:
        interaction.editReply({ content: "No War Report for that Hex found!" });
        break;
      case 200:
        console.log(
          `Api request for: WarReport, Response Code: ${statusCode}, File: Not Up-to Date!`
        );

        const data = (await response).data;

        fs.writeFile(
          path.resolve(
            __dirname,
            `../../cache/warReport/${mapName}${record.shardName}.json`
          ),
          JSON.stringify(data, null, 2),
          "utf-8",
          (err) => {
            if (err) {
              console.log(err);
            } else {
              console.log(`File: Updated!`);
            }
          }
        );

        const warReport = new EmbedBuilder()
          .addFields(
            {
              name: "Total Enlistments",
              value: `${data.totalEnlistments}`,
            },
            {
              name: "Colonial Casualties",
              value: `${data.colonialCasualties}`,
            },
            {
              name: "Warden Casualties",
              value: `${data.wardenCasualties}`,
            },
            {
              name: "Day Of War",
              value: `${data.dayOfWar}`,
            }
          )
          .setFooter({ text: "Requested at" })
          .setTimestamp(new Date());
        try {
          await interaction.editReply({ embeds: [warReport] });
        } catch (err) {
          console.log(err);
          await interaction.editReply({
            content:
              "Please enable all needed permisions. Or wait for an issue to be fixed. Support server: https://discord.gg/9wzppSgXdQ",
          });
        }
        break;
      case 304:
        console.log(
          `Api Request for: WarReport, Response Code: ${statusCode}, File: Up-to Date!`
        );

        const warReportCached = new EmbedBuilder()
          .addFields(
            {
              name: "Total Enlistments",
              value: `${cache.totalEnlistments}`,
            },
            {
              name: "Colonial Casualties",
              value: `${cache.colonialCasualties}`,
            },
            {
              name: "Warden Casualties",
              value: `${cache.wardenCasualties}`,
            },
            {
              name: "Day Of War",
              value: `${cache.dayOfWar}`,
            }
          )
          .setFooter({ text: "Requested at" })
          .setTimestamp(new Date());
        try {
          await interaction.editReply({ embeds: [warReportCached] });
        } catch (err) {
          console.log(err);
          await interaction.editReply({
            content:
              "Please enable all needed permisions. Or wait for an issue to be fixed. Support server: https://discord.gg/9wzppSgXdQ",
          });
        }
        break;
    }
  } catch (err) {
    console.log(err);
    await interaction.editReply({
      content:
        "Please enable all needed permisions. Or wait for an issue to be fixed. Support server: https://discord.gg/9wzppSgXdQ",
    });
  }
}
