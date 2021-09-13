module.exports = {
  apps: [
    {
      name: 'FoxholeWarBot',
      script: 'npm i && npm run start',
      watch: false,
      //cron_restart: "5 0 * * *",
      env: {
        TOKEN: '',
        PREFIX: 'War!',
      },
    },
  ],
};
