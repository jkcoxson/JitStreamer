// jkcoxson

const discord = require('discord.js');
const config = require('./config.json');
const TOKEN = config.token;

let client = new discord.Client({
    partials: [],
    intents: []
});

client.login(TOKEN);

client.on('ready', () => {
    console.log('Ready!');
    let command_req = require('./commands');
    let commands = new command_req(client);
    db_req = require('./database');
    client.database = new db_req();
});
