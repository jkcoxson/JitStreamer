// jkcoxson

const { BaseMessageComponent, MessageEmbed, MessageActionRow, MessageButton } = require('discord.js');

module.exports = class DevicesCommand {
    /**
     * @param {import('discord.js').Client} client
     */
    constructor(client) {
        this.client = client;
        this.name = 'devices';
        this.command = {
            name: 'devices',
            description: 'List all the devices you have registered',
        }
    }

    /**
     * Initializes the command
     */
    init() {
        // Create a new command
        console.log('Registering command: ' + this.name);
        this.postCommand();

        // Add a callback for the command
        this.client.on('interactionCreate', (interaction) => {
            if (interaction.isCommand() && interaction.command.name === 'devices') {
                let devices = this.client.database.get(interaction.member.user.id);
                if (devices === undefined) {
                    interaction.reply('You have not registered any devices yet');
                    return;
                }

                let msg = new MessageEmbed()
                    .setColor('#00FFFF')
                    .setTitle(interaction.member.user.username + '\'s Devices')
                interaction.reply({ content: '  ', embeds: [msg] }).then(() => {
                    devices.forEach(device => {
                        let msg = new MessageEmbed()
                            .setColor('#00FFFF')
                            .setTitle(device.name)
                            .setDescription(device.udid)
                        let row = new MessageActionRow()
                            .addComponents(
                                new MessageButton()
                                    .setCustomId(JSON.stringify({
                                        button: 'removeDevice',
                                        user: interaction.member.user.id,
                                        device: device.udid
                                    }))
                                    .setLabel('Remove')
                                    .setStyle('DANGER')
                            )
                        interaction.followUp({ content: '  ', embeds: [msg], components: [row] });
                    });
                });
            }

            if (interaction.isButton()) {
                let data = JSON.parse(interaction.customId);
                if (data.button === 'removeDevice') {
                    let devices = this.client.database.get(data.user);
                    let index = devices.findIndex(device => device.udid === data.device);
                    devices.splice(index, 1);
                    this.client.database.set(data.user, devices);
                    interaction.reply('Device removed');
                }
            }



        });
    }

    postCommand() {
        this.client.guilds.cache.forEach(guild => {
            this.client.application.commands.create(this.command, guild.id);
        });
    }

    /**@type {import('discord.js').Client} */
    client

    /**@type {String} */
    name

    command

}
