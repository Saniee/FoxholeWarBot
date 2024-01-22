import axios from "axios";
import fs from "node:fs";
import path from "node:path";

import PocketBase from "pocketbase";

import { DatabaseURL, CollectionName } from "../config.json";
import { CommandInteraction, Guild } from "discord.js";

export async function GenerateMapChoices() {
  // Able 'https://war-service-live.foxholeservices.com'
  // Baker 'https://war-service-live-2.foxholeservices.com'
  // Charlie 'https://war-service-live-3.foxholeservices.com'

  const servers = [
    { name: "Able", url: "https://war-service-live.foxholeservices.com" },
    { name: "Baker", url: "https://war-service-live-2.foxholeservices.com" },
    { name: "Charlie", url: "https://war-service-live-3.foxholeservices.com" },
  ];

  for (let i = 0; i < servers.length; i++) {
    const response = axios.get(`${servers[i].url}/api/worldconquest/maps`, {
      validateStatus: function (status) {
        return status < 600;
      },
    });

    const mapData = (await response).data;
    const responseStatus = (await response).status;

    switch (responseStatus) {
      case 503:
        console.log(
          `Server ${servers[i].name} not Online/Temporarily Unavailable.`
        );
        break;
      case 404:
        console.log(`Maps for the server ${servers[i].name} not found.`);
        break;
      case 200:
        fs.writeFile(
          path.resolve(
            __dirname,
            `./cache/mapChoices/${servers[i].name}Hexes.json`
          ),
          JSON.stringify(mapData, null, 2),
          "utf-8",
          (err) => {
            if (err) {
              console.log(err);
            } else {
              console.log(`Map Choices for ${servers[i].name} updated!`);
            }
          }
        );
    }
  }
}

export async function createNewDefaultRecord(
  pb: PocketBase,
  interaction?: CommandInteraction,
  guild?: Guild
) {
  var id;
  if (interaction) {
    id = interaction.guildId;
  } else if (guild) {
    id = guild.id;
  }
  var data = {
    guildId: id,
    shard: "https://war-service-live.foxholeservices.com",
    shardName: "Able",
    showCommandOutput: false,
  };

  await pb
    .collection(CollectionName)
    .create(data)
    .catch((err) => console.log(err));
}

export async function deleteRecord(pb: PocketBase, guild: Guild) {
  const record = await pb
    .collection(CollectionName)
    .getFirstListItem(`guildId=${guild.id}`)
    .catch((err) => {
      `No record for ${guild.name} found! (deleteRecordFunc)`;
    });
  if (record) {
    await pb.collection(CollectionName).delete(record.id);
  }
}

// TODO
// Fix Live Build | Potential Bug with PocketBase itself.
export async function PocketBaseLogin() {
  const pocketbase = new PocketBase(DatabaseURL);
  const authData = await pocketbase.admins
    .authWithPassword(***REMOVED***, ***REMOVED***)
    .then(() => console.log("Logged into PocketBase!"));

  return { pocketbase, authData };
}
