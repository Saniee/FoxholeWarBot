import { Canvas, createCanvas, loadImage } from 'canvas';
import axios from 'axios';
import serverConfig from '../mongodb/serverConfig';
import fs from 'fs';
import path from 'path';
import { Message } from 'discord.js/typings/index.js';

export = {
  make: async function (
    cache: any,
    mongoDB: any,
    dynamicMapDataUrl: any,
    args: string[],
    msg: Message
  ) {
    try {
      const canvas = createCanvas(1024, 888);

      const getDynamic = axios.get(dynamicMapDataUrl, {
        validateStatus: function (status) {
          return status < 500;
        },
        headers: { 'If-None-Match': `"${cache.version}"` },
      });

      const data: any = (await getDynamic).data;
      const statusCode: any = (await getDynamic).status;

      if (statusCode == 404) {
        msg.reply({
          content: 'There was nothing found for the Map/Hex Specified!',
        });
      } else if (statusCode == 200) {
        console.log(
          `Api request for: getMap, Response Code: ${statusCode}, File: Not Up-to Date!`
        );

        serverConfig.findOne(
          { guildID: msg.guildId },
          (err: any, mongoDB: any) => {
            fs.writeFile(
              path.resolve(
                __dirname,
                `../cache/getMap/${args[0]}${mongoDB.shardName}.json`
              ),
              JSON.stringify(data, null, 2),
              'utf-8',
              (err) => {
                if (err) {
                  console.log(err);
                } else {
                  console.log('File: Updated!');
                }
              }
            );
          }
        );

        const ctx = canvas.getContext('2d');
        const map = await loadImage(
          path.resolve(__dirname, `../assets/Images/MapsPNG/Map${args[0]}.png`)
        );
        ctx.drawImage(map, 0, 0, canvas.width, canvas.height);

        for (var i = 0; i < data.mapItems.length; i++) {
          if (data.mapItems[i].teamId == 'WARDENS') {
            const iconWardens = await loadImage(
              path.resolve(
                __dirname,
                `../assets/Images/MapIconsPNG/${data.mapItems[i].iconType}${data.mapItems[i].teamId}.png`
              )
            );
            // console.log(data.mapItems[i].iconType + '-' + `${iconWardens.src}`);
            ctx.drawImage(
              iconWardens,
              data.mapItems[i].x * 1000 * 1.005,
              data.mapItems[i].y * 1000 * 0.85,
              24,
              24
            );
          } else if (data.mapItems[i].teamId == 'COLONIALS') {
            const iconColonials = await loadImage(
              path.resolve(
                __dirname,
                `../assets/Images/MapIconsPNG/${data.mapItems[i].iconType}${data.mapItems[i].teamId}.png`
              )
            );
            // console.log(
            //   data.mapItems[i].iconType + '-' + `${iconColonials.src}`
            // );
            ctx.drawImage(
              iconColonials,
              data.mapItems[i].x * 1000 * 1.005,
              data.mapItems[i].y * 1000 * 0.85,
              24,
              24
            );
          } else {
            const iconNoTeam = await loadImage(
              path.resolve(
                __dirname,
                `../assets/Images/MapIconsPNG/${data.mapItems[i].iconType}.png`
              )
            );
            // console.log(data.mapItems[i].iconType + '-' + `${iconNoTeam.src}`);
            ctx.drawImage(
              iconNoTeam,
              data.mapItems[i].x * 1000 * 1.005,
              data.mapItems[i].y * 1000 * 0.85,
              24,
              24
            );
          }
        }
      } else if (statusCode == 304) {
        console.log(
          `Api request for: getMap, Response Code: ${statusCode}, File: Up-to Date!`
        );

        const data = cache;

        const ctx = canvas.getContext('2d');
        const map = await loadImage(
          path.resolve(__dirname, `../assets/Images/MapsPNG/Map${args[0]}.png`)
        );
        ctx.drawImage(map, 0, 0, canvas.width, canvas.height);

        for (var i = 0; i < data.mapItems.length; i++) {
          if (data.mapItems[i].teamId == 'WARDENS') {
            const iconWardens = await loadImage(
              path.resolve(
                __dirname,
                `../assets/Images/MapIconsPNG/${data.mapItems[i].iconType}${data.mapItems[i].teamId}.png`
              )
            );
            // console.log(data.mapItems[i].iconType + '-' + `${iconWardens.src}`);
            ctx.drawImage(
              iconWardens,
              data.mapItems[i].x * 1000 * 1.005,
              data.mapItems[i].y * 1000 * 0.85,
              24,
              24
            );
          } else if (data.mapItems[i].teamId == 'COLONIALS') {
            const iconColonials = await loadImage(
              path.resolve(
                __dirname,
                `../assets/Images/MapIconsPNG/${data.mapItems[i].iconType}${data.mapItems[i].teamId}.png`
              )
            );
            // console.log(
            //   data.mapItems[i].iconType + '-' + `${iconColonials.src}`
            // );
            ctx.drawImage(
              iconColonials,
              data.mapItems[i].x * 1000 * 1.005,
              data.mapItems[i].y * 1000 * 0.85,
              24,
              24
            );
          } else {
            const iconNoTeam = await loadImage(
              path.resolve(
                __dirname,
                `../assets/Images/MapIconsPNG/${data.mapItems[i].iconType}.png`
              )
            );
            // console.log(data.mapItems[i].iconType + '-' + `${iconNoTeam.src}`);
            ctx.drawImage(
              iconNoTeam,
              data.mapItems[i].x * 1000 * 1.005,
              data.mapItems[i].y * 1000 * 0.85,
              24,
              24
            );
          }
        }
      }
      return canvas;
    } catch (err) {
      console.log(err);
      msg.author
        .send(
          'Please enable all needed permisions. Or wait for an issue to be fixed. Support server: https://discord.gg/9wzppSgXdQ'
        )
        .catch((err) => {
          console.log(err);
        });
    }
  },
};
