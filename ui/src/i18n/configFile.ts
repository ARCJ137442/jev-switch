export const configFileEn = {
  'configFile.title': 'Configuration file sync',
  'configFile.summary': 'Import or export provider and route settings on the daemon machine.',
  'configFile.authority': 'Saved application settings are active. File edits take effect only after importing.',
  'configFile.changed': 'The TOML file has changed since its last import or export.',
  'configFile.unchanged': 'No external TOML changes detected.',
  'configFile.import': 'Import from file',
  'configFile.export': 'Write current settings to file',
  'configFile.importDetail': 'Replace the active providers and route graph with this TOML file. Public entry records and policies remain separate. Finish any open edits first.',
  'configFile.exportDetail': 'Overwrite the provider and route sections in this file on the daemon machine, including its configured upstream keys. No key is downloaded to this browser. Public entry records and policies are not a full database backup.',
  'configFile.imported': 'File settings imported.',
  'configFile.exported': 'Current settings written to the file.',
} as const;

export const configFileZh: Record<keyof typeof configFileEn, string> = {
  'configFile.title': '配置文件同步',
  'configFile.summary': '导入或导出内核所在机器上的提供商与路由配置。',
  'configFile.authority': '软件中已保存的配置正在生效。手动修改文件后，需要导入才会应用。',
  'configFile.changed': 'TOML 文件在上次导入或导出后发生了外部修改。',
  'configFile.unchanged': '未发现 TOML 文件的外部修改。',
  'configFile.import': '从文件导入',
  'configFile.export': '将当前配置写入文件',
  'configFile.importDetail': '以此 TOML 文件替换正在使用的提供商与路由图。对外入口记录及策略另行保留。请先完成正在进行的配置编辑。',
  'configFile.exportDetail': '覆盖内核所在机器上此文件的提供商和路由部分，包含已配置的上游密钥。密钥不会下载到浏览器；对外入口记录及策略仍在数据库中，此操作不是完整数据库备份。',
  'configFile.imported': '已导入文件配置。',
  'configFile.exported': '已将当前配置写入文件。',
};
