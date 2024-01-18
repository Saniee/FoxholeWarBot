import axios from "axios";
import fs from "node:fs";
import path from "node:path";

import PocketBase from "pocketbase";

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

// TODO
// Fix Live Build | Potential Bug with PocketBase itself.
export async function PocketBaseLogin() {
  const pocketbase = new PocketBase("http://127.0.0.1:8090");
  const authData = await pocketbase.admins
    .authWithPassword(***REMOVED***, "Axa@3436578")
    .then(() => console.log("Logged into PocketBase!"));

  return { pocketbase, authData };
}
