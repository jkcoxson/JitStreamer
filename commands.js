// jkcoxson

const fs = require('fs');
const path = require('path');

module.exports = class Commands {

    /**
     * @param {import('discord.js').Client} client
     */
    constructor(client) {
        this.client = client;
        // Require every single class in commands
        this.commands = [];
        fs.readdirSync(path.join(__dirname, 'commands')).forEach(file => {
            const command = require(path.join(__dirname, 'commands', file));
            this.commands.push(new command(this.client));
        });

        let commandNames = [];
        this.commands.forEach(command => {
            console.log('Initializing command: ' + command.name);
            commandNames.push(command.name);
            command.init();
        });

        this.client.application.commands.fetch().then(commands => {
            commands.forEach(command => {
                if (!commandNames.includes(command.name)) {
                    console.log('Class for command ' + command.name + ' not found. Deleting command.');
                    this.client.application.commands.delete(command);
                }
            });
        });

        this.client.on('guildCreate', () => {
            this.commands.forEach(command => {
                command.postCommand();
            });
        })

    }

    /**@type {import('discord.js').Client} */
    client

    /**@type {Array} */
    commands

}
