const axios = require('axios')
const fs = require('node:fs');
const path = require('node:path');

async function generate() {
    // Able 'https://war-service-live.foxholeservices.com'
    // Baker 'https://war-service-live-2.foxholeservices.com'
    // Charlie 'https://war-service-live-3.foxholeservices.com'

    const servers = [
        {name: 'Able', url: 'https://war-service-live.foxholeservices.com'},
        {name: 'Baker', url: 'https://war-service-live-2.foxholeservices.com'},
        {name: 'Charlie', url: 'https://war-service-live-3.foxholeservices.com'}
    ]

    for (let i = 0; i < servers.length; i++) {
        const response = axios.get(
            `${servers[i].url}/api/worldconquest/maps`,
            {
                validateStatus: function (status) {
                    return status < 600;
                },
            }
        );
        
        const mapData = (await response).data
        const responseStatus = (await response).status

        switch(responseStatus){
            case 503:
                console.log(`Server ${servers[i].name} not Online/Temporarily Unavailable.`,)
                break
            case 404:
                console.log(`Server ${servers[i].name} not found any maps.`,)
                break
            case 200:
                console.log(`Server ${servers[i].name} maps found! Saving...`)
                fs.writeFile(
                    path.resolve(
                        __dirname,
                        `./cache/mapChoices/${servers[i].name}Hexes.json`
                    ),
                    JSON.stringify(mapData, null, 2),
                    'utf-8',
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

exports.generate = generate;