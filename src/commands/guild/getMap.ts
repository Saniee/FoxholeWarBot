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
    .setName("get-map")
    .setDescription(
      "Responds with an image of that Hex/Map chunk. With Colored icons and labels."
    )
    .addStringOption((option) =>
      option
        .setName("map-name")
        .setDescription("Name of the Hex you want displayed.")
        .setRequired(true)
        .setAutocomplete(true)
    )
    .addBooleanOption((option) =>
      option
        .setName("render-labels")
        .setDescription(
          "Renders labels for things on the map. Turned off by default, since it makes the render messy."
        )
    )
    .addStringOption((option) =>
      option
        .setName("show-labels")
        .setDescription(
          "Sets what labels to render, default is Major like map names."
        )
        .addChoices(
          { name: "Major", value: "Major" },
          { name: "Minor", value: "Minor" },
          { name: "All", value: "All" }
        )
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

    var renderLabels = interaction.options.get("render-labels")?.value ?? false;

    var showLabels = interaction.options.get("show-labels")?.value;

    if (showLabels === null) {
      showLabels = "Major";
    }

    const record = await pb
      .collection(CollectionName)
      .getFirstListItem(`guildId=${interaction.guildId}`)
      .catch((err) =>
        console.log(`No record for ${interaction.guild?.name} found!`)
      );

    await interaction.deferReply({ ephemeral: true });

    if (record) {
      var dynamicCache;
      var staticCache;
      try {
        const fileData = fs.readFileSync(
          path.resolve(
            __dirname,
            `../../../cache/getMap/${mapName}${record.shardName}Dynamic.json`
          ),
          { encoding: "utf-8", flag: "r" }
        );
        dynamicCache = await JSON.parse(fileData);
      } catch (err) {
        dynamicCache = {
          version: "0",
        };
      }
      try {
        const fileData = fs.readFileSync(
          path.resolve(
            __dirname,
            `../../../cache/getMap/${mapName}${record.shardName}Static.json`
          ),
          { encoding: "utf-8", flag: "r" }
        );
        staticCache = await JSON.parse(fileData);
      } catch (err) {
        staticCache = {
          version: "0",
        };
      }

      if (
        fs.existsSync(
          path.resolve(
            __dirname,
            `../../assets/Images/MapsPNG/Map${mapName}.png`
          )
        )
      ) {
        await apiRequest(
          dynamicCache,
          staticCache,
          record,
          interaction,
          mapName,
          renderLabels,
          showLabels
        );
      } else {
        interaction.editReply(
          "A Hex/Map Png wasn't found! Either you entered a wrong name, the API doesn't have this map in the system or the bot hasnt been updated-- if so please go to the support server to request an update: https://discord.gg/9wzppSgXdQ"
        );
      }
    } else {
      await interaction.editReply({
        content:
          "Server setting missing, please run the command `/set-server` to fix this issue!",
      });
    }
  },
};

async function apiRequest(
  dynamicCache: any,
  staticCache: any,
  record: RecordModel,
  interaction: CommandInteraction,
  mapName: any,
  renderLabels: boolean,
  showLabels: string
) {
  const dynamicMapDataUrl = `${record.shard}/api/worldconquest/maps/${mapName}/dynamic/public`;
  const staticMapDataUrl = `${record.shard}/api/worldconquest/maps/${mapName}/static`;

  // headers: { 'If-None-Match': `"${dynamicCache.version}"` }

  try {
    const canvas = createCanvas(1024, 888);

    const getDymanic = axios.get(dynamicMapDataUrl, {
      validateStatus: function (status) {
        return status < 600;
      },
      headers: { "If-None-Match": `"${dynamicCache.version}"` },
    });

    const getStatic = axios.get(staticMapDataUrl, {
      validateStatus: function (status) {
        return status < 600;
      },
      headers: { "If-None-Match": `"${staticCache.version}"` },
    });

    const dataDynamic = (await getDymanic).data;
    const statusCodeDynamic = (await getDymanic).status;

    const dataStatic = (await getStatic).data;
    const statusCodeStatic = (await getStatic).status;

    // console.log(statusCodeDynamic + ' | ' + statusCodeStatic);

    if (statusCodeStatic == 503 || statusCodeDynamic == 503) {
      interaction.editReply({
        content:
          "Server not Online/Temporarily Unavailable. Please Change to Shard1/Shard2",
      });
    } else if (statusCodeDynamic == 404 || statusCodeStatic == 404) {
      interaction.editReply({
        content: "There was nothing found for the Map/Hex Specified!",
      });
    } else if (statusCodeDynamic == 200 && statusCodeStatic == 200) {
      console.log(
        `Api request for: getMap, Response Codes: ${statusCodeDynamic}|${statusCodeStatic}, File: Not Up-to Date!`
      );

      fs.writeFile(
        path.resolve(
          __dirname,
          `../../cache/getMap/${mapName}${record.shardName}Dynamic.json`
        ),
        JSON.stringify(dataDynamic, null, 2),
        "utf-8",
        (err) => {
          if (err) {
            console.log(err);
          } else {
            console.log("Dynamic Data: Updated!");
          }
        }
      );

      fs.writeFile(
        path.resolve(
          __dirname,
          `../../cache/getMap/${mapName}${record.shardName}Static.json`
        ),
        JSON.stringify(dataStatic, null, 2),
        "utf-8",
        (err) => {
          if (err) {
            console.log(err);
          } else {
            console.log("Static Data: Updated!");
          }
        }
      );

      const ctx = canvas.getContext("2d");

      const map = await loadImage(
        path.resolve(__dirname, `../../assets/Images/MapsPNG/Map${mapName}.png`)
      );
      ctx.drawImage(map, 0, 0, canvas.width, canvas.height);

      for (var i = 0; i < dataDynamic.mapItems.length; i++) {
        if (dataDynamic.mapItems[i].teamId == "WARDENS") {
          const iconWardens = await loadImage(
            path.resolve(
              __dirname,
              `../../assets/Images/MapIconsPNG/${dataDynamic.mapItems[i].iconType}${dataDynamic.mapItems[i].teamId}.png`
            )
          );
          // console.log(dataDynamic.mapItems[i].iconType + '-' + `${iconWardens.src}`);
          ctx.drawImage(
            iconWardens,
            dataDynamic.mapItems[i].x * 1000 * 1.005,
            dataDynamic.mapItems[i].y * 1000 * 0.85,
            24,
            24
          );
        } else if (dataDynamic.mapItems[i].teamId == "COLONIALS") {
          const iconColonials = await loadImage(
            path.resolve(
              __dirname,
              `../../assets/Images/MapIconsPNG/${dataDynamic.mapItems[i].iconType}${dataDynamic.mapItems[i].teamId}.png`
            )
          );
          // console.log(
          //   dataDynamic.mapItems[i].iconType + '-' + `${iconColonials.src}`
          // );
          ctx.drawImage(
            iconColonials,
            dataDynamic.mapItems[i].x * 1000 * 1.005,
            dataDynamic.mapItems[i].y * 1000 * 0.85,
            24,
            24
          );
        } else {
          if (
            fs.existsSync(
              path.resolve(
                __dirname,
                `../../assets/Images/MapIconsPNG/${dataDynamic.mapItems[i].iconType}.png`
              )
            )
          ) {
            const iconNoTeam = await loadImage(
              path.resolve(
                __dirname,
                `../../assets/Images/MapIconsPNG/${dataDynamic.mapItems[i].iconType}.png`
              )
            );
            // console.log(dataDynamic.mapItems[i].iconType + '-' + `${iconNoTeam.src}`);
            ctx.drawImage(
              iconNoTeam,
              dataDynamic.mapItems[i].x * 1000 * 1.005,
              dataDynamic.mapItems[i].y * 1000 * 0.85,
              24,
              24
            );
          }
        }
      }

      if (renderLabels == true) {
        for (var i = 0; i < dataStatic.mapTextItems.length; i++) {
          ctx.font = "20px serif";
          ctx.fillStyle = "#000000";
          if (
            dataStatic.mapTextItems[i].mapMarkerType === showLabels &&
            showLabels != "All"
          ) {
            ctx.fillText(
              `${dataStatic.mapTextItems[i].text}`,
              dataStatic.mapTextItems[i].x * 1000 * 0.9,
              dataStatic.mapTextItems[i].y * 1000 * 1
            );
          } else if (showLabels === "All") {
            ctx.fillText(
              `${dataStatic.mapTextItems[i].text}`,
              dataStatic.mapTextItems[i].x * 1000 * 0.9,
              dataStatic.mapTextItems[i].y * 1000 * 1
            );
          }
        }
      }

      await send(canvas, new Date(dataDynamic.lastUpdated), interaction);
    } else if (statusCodeDynamic == 304 && statusCodeStatic == 304) {
      console.log(
        `Api request for: getMap, Response Code: ${statusCodeDynamic}, File: Up-to Date!`
      );

      const dataDynamic = dynamicCache;
      const dataStatic = staticCache;

      const ctx = canvas.getContext("2d");

      const map = await loadImage(
        path.resolve(__dirname, `../../assets/Images/MapsPNG/Map${mapName}.png`)
      );
      ctx.drawImage(map, 0, 0, canvas.width, canvas.height);

      for (var i = 0; i < dataDynamic.mapItems.length; i++) {
        if (dataDynamic.mapItems[i].teamId == "WARDENS") {
          const iconWardens = await loadImage(
            path.resolve(
              __dirname,
              `../../assets/Images/MapIconsPNG/${dataDynamic.mapItems[i].iconType}${dataDynamic.mapItems[i].teamId}.png`
            )
          );
          // console.log(dataDynamic.mapItems[i].iconType + '-' + `${iconWardens.src}`);
          ctx.drawImage(
            iconWardens,
            dataDynamic.mapItems[i].x * 1000 * 1.005,
            dataDynamic.mapItems[i].y * 1000 * 0.85,
            24,
            24
          );
        } else if (dataDynamic.mapItems[i].teamId == "COLONIALS") {
          const iconColonials = await loadImage(
            path.resolve(
              __dirname,
              `../../assets/Images/MapIconsPNG/${dataDynamic.mapItems[i].iconType}${dataDynamic.mapItems[i].teamId}.png`
            )
          );
          // console.log(
          //   dataDynamic.mapItems[i].iconType + '-' + `${iconColonials.src}`
          // );
          ctx.drawImage(
            iconColonials,
            dataDynamic.mapItems[i].x * 1000 * 1.005,
            dataDynamic.mapItems[i].y * 1000 * 0.85,
            24,
            24
          );
        } else {
          if (
            fs.existsSync(
              path.resolve(
                __dirname,
                `../../assets/Images/MapIconsPNG/${dataDynamic.mapItems[i].iconType}.png`
              )
            )
          ) {
            const iconNoTeam = await loadImage(
              path.resolve(
                __dirname,
                `../../assets/Images/MapIconsPNG/${dataDynamic.mapItems[i].iconType}.png`
              )
            );
            // console.log(dataDynamic.mapItems[i].iconType + '-' + `${iconNoTeam.src}`);
            ctx.drawImage(
              iconNoTeam,
              dataDynamic.mapItems[i].x * 1000 * 1.005,
              dataDynamic.mapItems[i].y * 1000 * 0.85,
              24,
              24
            );
          }
        }
      }

      if (renderLabels == true) {
        for (var i = 0; i < dataStatic.mapTextItems.length; i++) {
          ctx.font = "20px serif";
          ctx.fillStyle = "#000000";
          if (
            dataStatic.mapTextItems[i].mapMarkerType === showLabels &&
            showLabels != "All"
          ) {
            ctx.fillText(
              `${dataStatic.mapTextItems[i].text}`,
              dataStatic.mapTextItems[i].x * 1000 * 0.9,
              dataStatic.mapTextItems[i].y * 1000 * 1
            );
          } else if (showLabels === "All") {
            ctx.fillText(
              `${dataStatic.mapTextItems[i].text}`,
              dataStatic.mapTextItems[i].x * 1000 * 0.9,
              dataStatic.mapTextItems[i].y * 1000 * 1
            );
          }
        }
      }

      await send(canvas, new Date(dynamicCache.lastUpdated), interaction);
    } else if (statusCodeDynamic != 200 && statusCodeStatic == 200) {
      console.log(
        `Api request for: getMap, Response Codes: ${statusCodeDynamic}|${statusCodeStatic}, File: Not Up-to Date!`
      );

      fs.writeFile(
        path.resolve(
          __dirname,
          `../../cache/getMap/${mapName}${record.shardName}Static.json`
        ),
        JSON.stringify(dataStatic, null, 2),
        "utf-8",
        (err) => {
          if (err) {
            console.log(err);
          } else {
            console.log("Static Data: Updated!");
          }
        }
      );

      const dataDynamic = dynamicCache;

      const ctx = canvas.getContext("2d");

      const map = await loadImage(
        path.resolve(__dirname, `../../assets/Images/MapsPNG/Map${mapName}.png`)
      );
      ctx.drawImage(map, 0, 0, canvas.width, canvas.height);

      for (var i = 0; i < dataDynamic.mapItems.length; i++) {
        if (dataDynamic.mapItems[i].teamId == "WARDENS") {
          const iconWardens = await loadImage(
            path.resolve(
              __dirname,
              `../../assets/Images/MapIconsPNG/${dataDynamic.mapItems[i].iconType}${dataDynamic.mapItems[i].teamId}.png`
            )
          );
          // console.log(dataDynamic.mapItems[i].iconType + '-' + `${iconWardens.src}`);
          ctx.drawImage(
            iconWardens,
            dataDynamic.mapItems[i].x * 1000 * 1.005,
            dataDynamic.mapItems[i].y * 1000 * 0.85,
            24,
            24
          );
        } else if (dataDynamic.mapItems[i].teamId == "COLONIALS") {
          const iconColonials = await loadImage(
            path.resolve(
              __dirname,
              `../../assets/Images/MapIconsPNG/${dataDynamic.mapItems[i].iconType}${dataDynamic.mapItems[i].teamId}.png`
            )
          );
          // console.log(
          //   dataDynamic.mapItems[i].iconType + '-' + `${iconColonials.src}`
          // );
          ctx.drawImage(
            iconColonials,
            dataDynamic.mapItems[i].x * 1000 * 1.005,
            dataDynamic.mapItems[i].y * 1000 * 0.85,
            24,
            24
          );
        } else {
          if (
            fs.existsSync(
              path.resolve(
                __dirname,
                `../../assets/Images/MapIconsPNG/${dataDynamic.mapItems[i].iconType}.png`
              )
            )
          ) {
            const iconNoTeam = await loadImage(
              path.resolve(
                __dirname,
                `../../assets/Images/MapIconsPNG/${dataDynamic.mapItems[i].iconType}.png`
              )
            );
            // console.log(dataDynamic.mapItems[i].iconType + '-' + `${iconNoTeam.src}`);
            ctx.drawImage(
              iconNoTeam,
              dataDynamic.mapItems[i].x * 1000 * 1.005,
              dataDynamic.mapItems[i].y * 1000 * 0.85,
              24,
              24
            );
          }
        }
      }

      if (renderLabels == true) {
        for (var i = 0; i < dataStatic.mapTextItems.length; i++) {
          ctx.font = "20px serif";
          ctx.fillStyle = "#000000";
          if (
            dataStatic.mapTextItems[i].mapMarkerType === showLabels &&
            showLabels != "All"
          ) {
            ctx.fillText(
              `${dataStatic.mapTextItems[i].text}`,
              dataStatic.mapTextItems[i].x * 1000 * 0.9,
              dataStatic.mapTextItems[i].y * 1000 * 1
            );
          } else if (showLabels === "All") {
            ctx.fillText(
              `${dataStatic.mapTextItems[i].text}`,
              dataStatic.mapTextItems[i].x * 1000 * 0.9,
              dataStatic.mapTextItems[i].y * 1000 * 1
            );
          }
        }
      }

      await send(canvas, new Date(dataDynamic.lastUpdated), interaction);
    } else if (statusCodeDynamic == 200 && statusCodeStatic != 200) {
      console.log(
        `Api request for: getMap, Response Codes: ${statusCodeDynamic}|${statusCodeStatic}, File: Not Up-to Date!`
      );

      fs.writeFile(
        path.resolve(
          __dirname,
          `../../cache/getMap/${mapName}${record.shardName}Dynamic.json`
        ),
        JSON.stringify(dataDynamic, null, 2),
        "utf-8",
        (err) => {
          if (err) {
            console.log(err);
          } else {
            console.log("Dynamic Data: Updated!");
          }
        }
      );

      const dataStatic = staticCache;

      const ctx = canvas.getContext("2d");

      const map = await loadImage(
        path.resolve(__dirname, `../../assets/Images/MapsPNG/Map${mapName}.png`)
      );
      ctx.drawImage(map, 0, 0, canvas.width, canvas.height);

      for (var i = 0; i < dataDynamic.mapItems.length; i++) {
        if (dataDynamic.mapItems[i].teamId == "WARDENS") {
          const iconWardens = await loadImage(
            path.resolve(
              __dirname,
              `../../assets/Images/MapIconsPNG/${dataDynamic.mapItems[i].iconType}${dataDynamic.mapItems[i].teamId}.png`
            )
          );
          // console.log(dataDynamic.mapItems[i].iconType + '-' + `${iconWardens.src}`);
          ctx.drawImage(
            iconWardens,
            dataDynamic.mapItems[i].x * 1000 * 1.005,
            dataDynamic.mapItems[i].y * 1000 * 0.85,
            24,
            24
          );
        } else if (dataDynamic.mapItems[i].teamId == "COLONIALS") {
          const iconColonials = await loadImage(
            path.resolve(
              __dirname,
              `../../assets/Images/MapIconsPNG/${dataDynamic.mapItems[i].iconType}${dataDynamic.mapItems[i].teamId}.png`
            )
          );
          // console.log(
          //   dataDynamic.mapItems[i].iconType + '-' + `${iconColonials.src}`
          // );
          ctx.drawImage(
            iconColonials,
            dataDynamic.mapItems[i].x * 1000 * 1.005,
            dataDynamic.mapItems[i].y * 1000 * 0.85,
            24,
            24
          );
        } else {
          if (
            fs.existsSync(
              path.resolve(
                __dirname,
                `../../assets/Images/MapIconsPNG/${dataDynamic.mapItems[i].iconType}.png`
              )
            )
          ) {
            const iconNoTeam = await loadImage(
              path.resolve(
                __dirname,
                `../../assets/Images/MapIconsPNG/${dataDynamic.mapItems[i].iconType}.png`
              )
            );
            // console.log(dataDynamic.mapItems[i].iconType + '-' + `${iconNoTeam.src}`);
            ctx.drawImage(
              iconNoTeam,
              dataDynamic.mapItems[i].x * 1000 * 1.005,
              dataDynamic.mapItems[i].y * 1000 * 0.85,
              24,
              24
            );
          }
        }
      }

      if (renderLabels == true) {
        for (var i = 0; i < dataStatic.mapTextItems.length; i++) {
          ctx.font = "20px serif";
          ctx.fillStyle = "#000000";
          if (
            dataStatic.mapTextItems[i].mapMarkerType === showLabels &&
            showLabels != "All"
          ) {
            ctx.fillText(
              `${dataStatic.mapTextItems[i].text}`,
              dataStatic.mapTextItems[i].x * 1000 * 0.9,
              dataStatic.mapTextItems[i].y * 1000 * 1
            );
          } else if (showLabels === "All") {
            ctx.fillText(
              `${dataStatic.mapTextItems[i].text}`,
              dataStatic.mapTextItems[i].x * 1000 * 0.9,
              dataStatic.mapTextItems[i].y * 1000 * 1
            );
          }
        }
      }

      await send(canvas, new Date(dataDynamic.lastUpdated), interaction);
    }
  } catch (err) {
    console.log(err);
    await interaction.editReply({
      content:
        "Please enable all needed permisions. Or wait for an issue to be fixed. Support server: https://discord.gg/9wzppSgXdQ",
    });
  }
}

/**
 *
 * @param {Canvas} canvas
 * @param {Date} lastUpdated
 * @param {CommandInteraction} interaction
 */
async function send(canvas, lastUpdated, interaction) {
  const attachment = new AttachmentBuilder(canvas.toBuffer(), {
    name: "image.png",
  });

  const myEmbed = new EmbedBuilder()
    .setDescription(`Last API Update: ` + `${lastUpdated}`)
    .setColor(Colors.Green)
    .setImage("attachment://image.png")
    .setFooter({ text: "Requested at" })
    .setTimestamp(new Date());
  try {
    await interaction.editReply({ embeds: [myEmbed], files: [attachment] });
  } catch (err) {
    console.log(err);
    await interaction.editReply({
      content:
        "Please enable all needed permisions. Or wait for an issue to be fixed. Support server: https://discord.gg/9wzppSgXdQ",
    });
  }
}
