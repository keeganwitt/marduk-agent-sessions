import { defineMessages } from "../catalog";

/**
 * The Help dialog. Prose may contain `<strong>`, `<em>` and `<code>` tags, which the dialog
 * renders as markup, so translations can put emphasis wherever their word order needs it.
 */
export const help = defineMessages(
  {
    "help.title": "How Marduk changes sessions",
    "help.topics": "Help topics",
    "help.tab.deleting": "Deleting",
    "help.tab.archiving": "Archiving",
    "help.tab.safety": "Safety",
    "help.tab.subagents": "Subagents & labels",
    "help.tab.agents": "Agents",
    "help.deleting.before.title": "Before anything happens",
    "help.deleting.before.body":
      "<strong>Delete…</strong> lists every file that will be moved and every command that will run. Nothing changes until you confirm.",
    "help.deleting.trash.title": "Trash",
    "help.deleting.trash.body":
      "Agents without a delete command of their own have their files moved to the system Trash (the Recycle Bin on Windows), where you can restore them. If a file can't be moved to the Trash, Marduk reports the error rather than deleting it permanently.",
    "help.deleting.permanent.title": "Permanent",
    "help.deleting.permanent.body":
      "When an agent has its own delete command, Marduk runs it, so the agent's own indexes stay consistent. These deletes can't be undone. The confirmation marks them <em>Permanent</em> and shows the exact command.",
    "help.deleting.seeAgents":
      "The <strong>Agents</strong> tab says which applies to each agent.",
    "help.archiving.agent.title": "Only through the agent",
    "help.archiving.agent.body":
      "<strong>Archive</strong> is offered only for agents with their own archive command, and runs that command. The session stays in the agent, which can still open it.",
    "help.archiving.none.title": "No invented archives",
    "help.archiving.none.body":
      "Marduk never makes archive folders that an agent doesn't know about. For agents without an archive, delete or keep the session instead.",
    "help.safety.title": "What Marduk refuses to do",
    "help.safety.running":
      "Change a <em>Running</em> session, one written to in the last 5 minutes.",
    "help.safety.changed":
      "Change a session that changed after the last scan. Rescan, then try again.",
    "help.safety.outside":
      "Touch files outside the agent's own session folders.",
    "help.safety.database":
      "Write to an agent's database. Database-backed agents are changed only through their own command-line tool, and are read-only without it.",
    "help.safety.install":
      "Install or update an agent's command-line tool. Choose where yours is in <strong>Settings</strong>.",
    "help.subagents.title": "Subagents",
    "help.subagents.body":
      "Sessions an agent started on its own are nested under the session that started them; expand the parent to see them. Some agents delete them along with the parent, and the confirmation says when that happens. A subagent whose parent is gone shows <em>Parent deleted</em>.",
    "help.labels.title": "Labels",
    "help.labels.body":
      "Labels are kept by Marduk, not the agent, so they survive archiving. Deleting a session in Marduk removes its labels. Labels of sessions deleted some other way are removed once the session has been missing for 7 days.",
    "help.agents.loading": "Loading…",
    "help.agents.none": "No agents.",
    "help.agents.list": "Agent",
    "help.agents.delete": "Delete",
    "help.agents.archive": "Archive",
    "help.agents.reads": "Reads sessions from",
    "help.done": "Done",
  },
  {
    es: {
      "help.title": "Cómo cambia Marduk las sesiones",
      "help.topics": "Temas de ayuda",
      "help.tab.deleting": "Eliminar",
      "help.tab.archiving": "Archivar",
      "help.tab.safety": "Seguridad",
      "help.tab.subagents": "Subagentes y etiquetas",
      "help.tab.agents": "Agentes",
      "help.deleting.before.title": "Antes de que pase nada",
      "help.deleting.before.body":
        "<strong>Eliminar…</strong> muestra cada archivo que se moverá y cada comando que se ejecutará. Nada cambia hasta que lo confirmes.",
      "help.deleting.trash.title": "Papelera",
      "help.deleting.trash.body":
        "Los agentes sin un comando propio para eliminar mueven sus archivos a la Papelera del sistema (la Papelera de reciclaje en Windows), desde donde puedes restaurarlos. Si un archivo no se puede mover a la Papelera, Marduk informa del error en lugar de eliminarlo permanentemente.",
      "help.deleting.permanent.title": "Permanente",
      "help.deleting.permanent.body":
        "Cuando un agente tiene su propio comando para eliminar, Marduk lo ejecuta para que los índices del agente sigan siendo coherentes. Estas eliminaciones no se pueden deshacer. La confirmación las marca como <em>Permanente</em> y muestra el comando exacto.",
      "help.deleting.seeAgents":
        "La pestaña <strong>Agentes</strong> indica qué se aplica a cada agente.",
      "help.archiving.agent.title": "Solo a través del agente",
      "help.archiving.agent.body":
        "<strong>Archivar</strong> solo se ofrece para agentes con su propio comando de archivo, y ejecuta ese comando. La sesión sigue en el agente, que todavía puede abrirla.",
      "help.archiving.none.title": "Sin archivos inventados",
      "help.archiving.none.body":
        "Marduk nunca crea carpetas de archivo que un agente no conozca. Para los agentes sin archivo, elimina la sesión o consérvala.",
      "help.safety.title": "Lo que Marduk se niega a hacer",
      "help.safety.running":
        "Cambiar una sesión <em>En ejecución</em>, es decir, escrita en los últimos 5 minutos.",
      "help.safety.changed":
        "Cambiar una sesión que cambió después del último escaneo. Vuelve a escanear y luego inténtalo de nuevo.",
      "help.safety.outside":
        "Tocar archivos fuera de las carpetas de sesiones del propio agente.",
      "help.safety.database":
        "Escribir en la base de datos de un agente. Los agentes con base de datos solo se cambian a través de su propia herramienta de línea de comandos y son de solo lectura sin ella.",
      "help.safety.install":
        "Instalar o actualizar la herramienta de línea de comandos de un agente. Elige dónde está la tuya en <strong>Ajustes</strong>.",
      "help.subagents.title": "Subagentes",
      "help.subagents.body":
        "Las sesiones que un agente inició por su cuenta aparecen anidadas bajo la sesión que las inició; despliega la principal para verlas. Algunos agentes las eliminan junto con la principal, y la confirmación indica cuándo ocurre. Un subagente cuya sesión principal ya no existe muestra <em>Sesión principal eliminada</em>.",
      "help.labels.title": "Etiquetas",
      "help.labels.body":
        "Las etiquetas las guarda Marduk, no el agente, así que sobreviven al archivado. Eliminar una sesión en Marduk elimina sus etiquetas. Las etiquetas de sesiones eliminadas de otra forma se quitan cuando la sesión lleva 7 días sin aparecer.",
      "help.agents.loading": "Cargando…",
      "help.agents.none": "No hay agentes.",
      "help.agents.list": "Agente",
      "help.agents.delete": "Eliminar",
      "help.agents.archive": "Archivar",
      "help.agents.reads": "Lee sesiones de",
      "help.done": "Listo",
    },
    "pt-BR": {
      "help.title": "Como o Marduk altera as sessões",
      "help.topics": "Tópicos de ajuda",
      "help.tab.deleting": "Excluir",
      "help.tab.archiving": "Arquivar",
      "help.tab.safety": "Segurança",
      "help.tab.subagents": "Subagentes e etiquetas",
      "help.tab.agents": "Agentes",
      "help.deleting.before.title": "Antes de qualquer coisa",
      "help.deleting.before.body":
        "<strong>Excluir…</strong> lista cada arquivo que será movido e cada comando que será executado. Nada muda até você confirmar.",
      "help.deleting.trash.title": "Lixeira",
      "help.deleting.trash.body":
        "Agentes sem um comando próprio de exclusão têm seus arquivos movidos para a Lixeira do sistema (a Lixeira no Windows), de onde você pode restaurá-los. Se um arquivo não puder ser movido para a Lixeira, o Marduk informa o erro em vez de excluí-lo permanentemente.",
      "help.deleting.permanent.title": "Permanente",
      "help.deleting.permanent.body":
        "Quando um agente tem seu próprio comando de exclusão, o Marduk o executa para que os índices do agente continuem consistentes. Essas exclusões não podem ser desfeitas. A confirmação as marca como <em>Permanente</em> e mostra o comando exato.",
      "help.deleting.seeAgents":
        "A aba <strong>Agentes</strong> informa o que vale para cada agente.",
      "help.archiving.agent.title": "Somente pelo agente",
      "help.archiving.agent.body":
        "<strong>Arquivar</strong> só é oferecido para agentes com seu próprio comando de arquivamento e executa esse comando. A sessão continua no agente, que ainda pode abri-la.",
      "help.archiving.none.title": "Nada de arquivos inventados",
      "help.archiving.none.body":
        "O Marduk nunca cria pastas de arquivo que um agente não conheça. Para agentes sem arquivamento, exclua ou mantenha a sessão.",
      "help.safety.title": "O que o Marduk se recusa a fazer",
      "help.safety.running":
        "Alterar uma sessão <em>Em execução</em>, ou seja, gravada nos últimos 5 minutos.",
      "help.safety.changed":
        "Alterar uma sessão que mudou depois da última verificação. Verifique novamente e tente outra vez.",
      "help.safety.outside":
        "Mexer em arquivos fora das pastas de sessões do próprio agente.",
      "help.safety.database":
        "Gravar no banco de dados de um agente. Agentes com banco de dados só são alterados pela própria ferramenta de linha de comando e ficam somente leitura sem ela.",
      "help.safety.install":
        "Instalar ou atualizar a ferramenta de linha de comando de um agente. Escolha onde está a sua em <strong>Configurações</strong>.",
      "help.subagents.title": "Subagentes",
      "help.subagents.body":
        "Sessões que um agente iniciou por conta própria aparecem aninhadas sob a sessão que as iniciou; expanda a principal para vê-las. Alguns agentes as excluem junto com a principal, e a confirmação avisa quando isso acontece. Um subagente cuja sessão principal não existe mais mostra <em>Sessão principal excluída</em>.",
      "help.labels.title": "Etiquetas",
      "help.labels.body":
        "As etiquetas são guardadas pelo Marduk, não pelo agente, então sobrevivem ao arquivamento. Excluir uma sessão no Marduk remove suas etiquetas. Etiquetas de sessões excluídas de outra forma são removidas quando a sessão está ausente há 7 dias.",
      "help.agents.loading": "Carregando…",
      "help.agents.none": "Nenhum agente.",
      "help.agents.list": "Agente",
      "help.agents.delete": "Excluir",
      "help.agents.archive": "Arquivar",
      "help.agents.reads": "Lê sessões de",
      "help.done": "Concluído",
    },
    fr: {
      "help.title": "Comment Marduk modifie les sessions",
      "help.topics": "Rubriques d'aide",
      "help.tab.deleting": "Suppression",
      "help.tab.archiving": "Archivage",
      "help.tab.safety": "Sécurité",
      "help.tab.subagents": "Sous-agents et étiquettes",
      "help.tab.agents": "Agents",
      "help.deleting.before.title": "Avant toute action",
      "help.deleting.before.body":
        "<strong>Supprimer…</strong> liste chaque fichier qui sera déplacé et chaque commande qui sera exécutée. Rien ne change tant que vous n'avez pas confirmé.",
      "help.deleting.trash.title": "Corbeille",
      "help.deleting.trash.body":
        "Les agents sans commande de suppression propre voient leurs fichiers déplacés vers la Corbeille du système (la Corbeille sous Windows), d'où vous pouvez les restaurer. Si un fichier ne peut pas être déplacé vers la Corbeille, Marduk signale l'erreur au lieu de le supprimer définitivement.",
      "help.deleting.permanent.title": "Définitive",
      "help.deleting.permanent.body":
        "Quand un agent a sa propre commande de suppression, Marduk l'exécute afin que les index de l'agent restent cohérents. Ces suppressions sont irréversibles. La confirmation les marque comme <em>Définitive</em> et affiche la commande exacte.",
      "help.deleting.seeAgents":
        "L'onglet <strong>Agents</strong> indique ce qui s'applique à chaque agent.",
      "help.archiving.agent.title": "Uniquement via l'agent",
      "help.archiving.agent.body":
        "<strong>Archiver</strong> n'est proposé que pour les agents dotés de leur propre commande d'archivage, et exécute cette commande. La session reste dans l'agent, qui peut toujours l'ouvrir.",
      "help.archiving.none.title": "Pas d'archives inventées",
      "help.archiving.none.body":
        "Marduk ne crée jamais de dossiers d'archives qu'un agent ne connaît pas. Pour les agents sans archivage, supprimez ou conservez plutôt la session.",
      "help.safety.title": "Ce que Marduk refuse de faire",
      "help.safety.running":
        "Modifier une session <em>En cours</em>, c'est-à-dire écrite au cours des 5 dernières minutes.",
      "help.safety.changed":
        "Modifier une session qui a changé depuis la dernière analyse. Réanalysez, puis réessayez.",
      "help.safety.outside":
        "Toucher à des fichiers en dehors des dossiers de sessions de l'agent.",
      "help.safety.database":
        "Écrire dans la base de données d'un agent. Les agents qui utilisent une base de données ne sont modifiés que par leur propre outil en ligne de commande, et sont en lecture seule sans lui.",
      "help.safety.install":
        "Installer ou mettre à jour l'outil en ligne de commande d'un agent. Indiquez où se trouve le vôtre dans <strong>Réglages</strong>.",
      "help.subagents.title": "Sous-agents",
      "help.subagents.body":
        "Les sessions qu'un agent a lancées de lui-même sont imbriquées sous la session qui les a lancées ; développez la session parente pour les voir. Certains agents les suppriment avec la session parente, et la confirmation l'indique. Un sous-agent dont la session parente n'existe plus affiche <em>Session parente supprimée</em>.",
      "help.labels.title": "Étiquettes",
      "help.labels.body":
        "Les étiquettes sont conservées par Marduk, pas par l'agent, elles survivent donc à l'archivage. Supprimer une session dans Marduk supprime ses étiquettes. Les étiquettes des sessions supprimées autrement sont retirées une fois la session absente depuis 7 jours.",
      "help.agents.loading": "Chargement…",
      "help.agents.none": "Aucun agent.",
      "help.agents.list": "Agent",
      "help.agents.delete": "Suppression",
      "help.agents.archive": "Archivage",
      "help.agents.reads": "Lit les sessions depuis",
      "help.done": "Terminé",
    },
    de: {
      "help.title": "Wie Marduk Sitzungen ändert",
      "help.topics": "Hilfethemen",
      "help.tab.deleting": "Löschen",
      "help.tab.archiving": "Archivieren",
      "help.tab.safety": "Sicherheit",
      "help.tab.subagents": "Subagents & Labels",
      "help.tab.agents": "Agents",
      "help.deleting.before.title": "Bevor etwas passiert",
      "help.deleting.before.body":
        "<strong>Löschen…</strong> listet jede Datei auf, die verschoben wird, und jeden Befehl, der ausgeführt wird. Nichts ändert sich, bevor Sie bestätigen.",
      "help.deleting.trash.title": "Papierkorb",
      "help.deleting.trash.body":
        "Bei Agents ohne eigenen Löschbefehl werden die Dateien in den Papierkorb des Systems verschoben (unter Windows den Papierkorb), wo Sie sie wiederherstellen können. Lässt sich eine Datei nicht in den Papierkorb verschieben, meldet Marduk den Fehler, statt sie endgültig zu löschen.",
      "help.deleting.permanent.title": "Endgültig",
      "help.deleting.permanent.body":
        "Hat ein Agent einen eigenen Löschbefehl, führt Marduk ihn aus, damit die Indizes des Agents stimmig bleiben. Diese Löschvorgänge lassen sich nicht rückgängig machen. Die Bestätigung kennzeichnet sie als <em>Endgültig</em> und zeigt den genauen Befehl.",
      "help.deleting.seeAgents":
        "Auf dem Tab <strong>Agents</strong> steht, was für welchen Agent gilt.",
      "help.archiving.agent.title": "Nur über den Agent",
      "help.archiving.agent.body":
        "<strong>Archivieren</strong> wird nur für Agents mit eigenem Archivbefehl angeboten und führt diesen Befehl aus. Die Sitzung bleibt im Agent, der sie weiterhin öffnen kann.",
      "help.archiving.none.title": "Keine erfundenen Archive",
      "help.archiving.none.body":
        "Marduk legt nie Archivordner an, die ein Agent nicht kennt. Bei Agents ohne Archiv löschen oder behalten Sie die Sitzung stattdessen.",
      "help.safety.title": "Was Marduk nicht tut",
      "help.safety.running":
        "Eine Sitzung ändern, die <em>Läuft</em>, also in den letzten 5 Minuten geschrieben wurde.",
      "help.safety.changed":
        "Eine Sitzung ändern, die sich seit dem letzten Scan geändert hat. Scannen Sie erneut und versuchen Sie es dann noch einmal.",
      "help.safety.outside":
        "Dateien außerhalb der Sitzungsordner des Agents anfassen.",
      "help.safety.database":
        "In die Datenbank eines Agents schreiben. Datenbankbasierte Agents werden nur über ihr eigenes Kommandozeilenwerkzeug geändert und sind ohne es schreibgeschützt.",
      "help.safety.install":
        "Das Kommandozeilenwerkzeug eines Agents installieren oder aktualisieren. Wo Ihres liegt, wählen Sie in den <strong>Einstellungen</strong>.",
      "help.subagents.title": "Subagents",
      "help.subagents.body":
        "Sitzungen, die ein Agent selbst gestartet hat, erscheinen unter der Sitzung, die sie gestartet hat; klappen Sie die übergeordnete Sitzung auf, um sie zu sehen. Manche Agents löschen sie zusammen mit der übergeordneten Sitzung, und die Bestätigung weist darauf hin. Ein Subagent, dessen übergeordnete Sitzung fehlt, zeigt <em>Übergeordnete Sitzung gelöscht</em>.",
      "help.labels.title": "Labels",
      "help.labels.body":
        "Labels verwaltet Marduk, nicht der Agent, daher bleiben sie beim Archivieren erhalten. Wenn Sie eine Sitzung in Marduk löschen, werden ihre Labels entfernt. Labels von Sitzungen, die anderweitig gelöscht wurden, werden entfernt, sobald die Sitzung 7 Tage lang fehlt.",
      "help.agents.loading": "Wird geladen…",
      "help.agents.none": "Keine Agents.",
      "help.agents.list": "Agent",
      "help.agents.delete": "Löschen",
      "help.agents.archive": "Archivieren",
      "help.agents.reads": "Liest Sitzungen aus",
      "help.done": "Fertig",
    },
    ja: {
      "help.title": "Marduk がセッションを変更する仕組み",
      "help.topics": "ヘルプのトピック",
      "help.tab.deleting": "削除",
      "help.tab.archiving": "アーカイブ",
      "help.tab.safety": "安全性",
      "help.tab.subagents": "サブエージェントとラベル",
      "help.tab.agents": "エージェント",
      "help.deleting.before.title": "実行する前に",
      "help.deleting.before.body":
        "<strong>削除…</strong> では、移動されるすべてのファイルと実行されるすべてのコマンドが一覧表示されます。確認するまで何も変更されません。",
      "help.deleting.trash.title": "ゴミ箱",
      "help.deleting.trash.body":
        "独自の削除コマンドを持たないエージェントのファイルは、システムのゴミ箱（Windows ではごみ箱）に移動され、そこから復元できます。ファイルをゴミ箱に移動できない場合、Marduk は完全に削除せずにエラーを報告します。",
      "help.deleting.permanent.title": "完全削除",
      "help.deleting.permanent.body":
        "エージェントに独自の削除コマンドがある場合、エージェントのインデックスの整合性を保つため Marduk はそのコマンドを実行します。この削除は元に戻せません。確認画面では <em>完全削除</em> と表示され、実行するコマンドがそのまま示されます。",
      "help.deleting.seeAgents":
        "どちらが適用されるかは <strong>エージェント</strong> タブで確認できます。",
      "help.archiving.agent.title": "エージェント経由のみ",
      "help.archiving.agent.body":
        "<strong>アーカイブ</strong> は独自のアーカイブコマンドを持つエージェントにのみ表示され、そのコマンドを実行します。セッションはエージェント内に残り、引き続き開くことができます。",
      "help.archiving.none.title": "独自のアーカイブは作りません",
      "help.archiving.none.body":
        "Marduk は、エージェントが認識しないアーカイブフォルダを作成しません。アーカイブ機能のないエージェントでは、セッションを削除するかそのまま残してください。",
      "help.safety.title": "Marduk が行わないこと",
      "help.safety.running":
        "<em>実行中</em> のセッション（直近 5 分以内に書き込まれたもの）の変更。",
      "help.safety.changed":
        "前回のスキャン後に変更されたセッションの変更。再スキャンしてからもう一度お試しください。",
      "help.safety.outside":
        "エージェント自身のセッションフォルダ以外にあるファイルへのアクセス。",
      "help.safety.database":
        "エージェントのデータベースへの書き込み。データベースを使うエージェントは独自のコマンドラインツール経由でのみ変更され、ツールがない場合は読み取り専用になります。",
      "help.safety.install":
        "エージェントのコマンドラインツールのインストールや更新。ツールの場所は <strong>設定</strong> で選択できます。",
      "help.subagents.title": "サブエージェント",
      "help.subagents.body":
        "エージェントが自ら開始したセッションは、それを開始したセッションの下にまとめて表示されます。親セッションを展開すると表示されます。親セッションと一緒に削除するエージェントもあり、その場合は確認画面に表示されます。親セッションがなくなったサブエージェントには <em>親セッション削除済み</em> と表示されます。",
      "help.labels.title": "ラベル",
      "help.labels.body":
        "ラベルはエージェントではなく Marduk が保持するため、アーカイブしても残ります。Marduk でセッションを削除するとラベルも削除されます。ほかの方法で削除されたセッションのラベルは、そのセッションが 7 日間見つからなかった時点で削除されます。",
      "help.agents.loading": "読み込み中…",
      "help.agents.none": "エージェントはありません。",
      "help.agents.list": "エージェント",
      "help.agents.delete": "削除",
      "help.agents.archive": "アーカイブ",
      "help.agents.reads": "セッションの読み取り元",
      "help.done": "完了",
    },
    ko: {
      "help.title": "Marduk이 세션을 변경하는 방식",
      "help.topics": "도움말 주제",
      "help.tab.deleting": "삭제",
      "help.tab.archiving": "보관",
      "help.tab.safety": "안전",
      "help.tab.subagents": "하위 에이전트 및 라벨",
      "help.tab.agents": "에이전트",
      "help.deleting.before.title": "실행하기 전에",
      "help.deleting.before.body":
        "<strong>삭제…</strong>는 이동될 모든 파일과 실행될 모든 명령을 보여 줍니다. 확인하기 전에는 아무것도 바뀌지 않습니다.",
      "help.deleting.trash.title": "휴지통",
      "help.deleting.trash.body":
        "자체 삭제 명령이 없는 에이전트의 파일은 시스템 휴지통(Windows에서는 휴지통)으로 이동하며, 그곳에서 복원할 수 있습니다. 파일을 휴지통으로 옮길 수 없으면 Marduk은 영구 삭제하지 않고 오류를 알립니다.",
      "help.deleting.permanent.title": "영구 삭제",
      "help.deleting.permanent.body":
        "에이전트에 자체 삭제 명령이 있으면 Marduk은 에이전트의 인덱스가 일관되게 유지되도록 그 명령을 실행합니다. 이 삭제는 되돌릴 수 없습니다. 확인 창에 <em>영구 삭제</em>로 표시되고 실행할 명령이 그대로 나옵니다.",
      "help.deleting.seeAgents":
        "에이전트별로 어떤 방식이 적용되는지는 <strong>에이전트</strong> 탭에서 볼 수 있습니다.",
      "help.archiving.agent.title": "에이전트를 통해서만",
      "help.archiving.agent.body":
        "<strong>보관</strong>은 자체 보관 명령이 있는 에이전트에만 제공되며 그 명령을 실행합니다. 세션은 에이전트에 남아 있어 계속 열 수 있습니다.",
      "help.archiving.none.title": "임의의 보관함은 만들지 않음",
      "help.archiving.none.body":
        "Marduk은 에이전트가 모르는 보관 폴더를 만들지 않습니다. 보관 기능이 없는 에이전트는 세션을 삭제하거나 그대로 두세요.",
      "help.safety.title": "Marduk이 하지 않는 일",
      "help.safety.running":
        "<em>실행 중</em>인 세션, 즉 최근 5분 안에 기록된 세션 변경.",
      "help.safety.changed":
        "마지막 스캔 이후 바뀐 세션 변경. 다시 스캔한 후 다시 시도하세요.",
      "help.safety.outside": "에이전트 자체 세션 폴더 밖의 파일 건드리기.",
      "help.safety.database":
        "에이전트 데이터베이스에 쓰기. 데이터베이스를 쓰는 에이전트는 자체 명령줄 도구로만 변경되며, 도구가 없으면 읽기 전용입니다.",
      "help.safety.install":
        "에이전트의 명령줄 도구 설치 또는 업데이트. 도구 위치는 <strong>설정</strong>에서 선택하세요.",
      "help.subagents.title": "하위 에이전트",
      "help.subagents.body":
        "에이전트가 스스로 시작한 세션은 그 세션을 시작한 세션 아래에 묶여 표시됩니다. 상위 세션을 펼치면 볼 수 있습니다. 일부 에이전트는 상위 세션과 함께 삭제하며, 그런 경우 확인 창에 표시됩니다. 상위 세션이 없어진 하위 에이전트에는 <em>상위 세션 삭제됨</em>이 표시됩니다.",
      "help.labels.title": "라벨",
      "help.labels.body":
        "라벨은 에이전트가 아니라 Marduk이 보관하므로 보관해도 그대로 남습니다. Marduk에서 세션을 삭제하면 라벨도 삭제됩니다. 다른 방법으로 삭제된 세션의 라벨은 세션이 7일 동안 보이지 않으면 삭제됩니다.",
      "help.agents.loading": "불러오는 중…",
      "help.agents.none": "에이전트가 없습니다.",
      "help.agents.list": "에이전트",
      "help.agents.delete": "삭제",
      "help.agents.archive": "보관",
      "help.agents.reads": "세션을 읽는 위치",
      "help.done": "완료",
    },
    "zh-Hans": {
      "help.title": "Marduk 如何更改会话",
      "help.topics": "帮助主题",
      "help.tab.deleting": "删除",
      "help.tab.archiving": "归档",
      "help.tab.safety": "安全",
      "help.tab.subagents": "子智能体和标签",
      "help.tab.agents": "智能体",
      "help.deleting.before.title": "在任何操作之前",
      "help.deleting.before.body":
        "<strong>删除…</strong>会列出将要移动的每个文件和将要运行的每条命令。在你确认之前不会有任何更改。",
      "help.deleting.trash.title": "废纸篓",
      "help.deleting.trash.body":
        "没有自带删除命令的智能体，其文件会被移到系统废纸篓（Windows 上为回收站），你可以从那里恢复。如果某个文件无法移到废纸篓，Marduk 会报告错误，而不会将其永久删除。",
      "help.deleting.permanent.title": "永久",
      "help.deleting.permanent.body":
        "如果智能体有自带的删除命令，Marduk 会运行该命令，以保持智能体自身索引的一致。这类删除无法撤销。确认窗口会将其标记为<em>永久</em>，并显示确切的命令。",
      "help.deleting.seeAgents":
        "<strong>智能体</strong>选项卡会说明每个智能体适用哪种方式。",
      "help.archiving.agent.title": "仅通过智能体",
      "help.archiving.agent.body":
        "只有自带归档命令的智能体才提供<strong>归档</strong>，并运行该命令。会话仍保留在智能体中，智能体仍可打开它。",
      "help.archiving.none.title": "不自创归档",
      "help.archiving.none.body":
        "Marduk 从不创建智能体不知道的归档文件夹。对于没有归档功能的智能体，请删除或保留会话。",
      "help.safety.title": "Marduk 拒绝执行的操作",
      "help.safety.running":
        "更改<em>运行中</em>的会话，即最近 5 分钟内写入过的会话。",
      "help.safety.changed":
        "更改上次扫描后发生变化的会话。请重新扫描，然后再试一次。",
      "help.safety.outside": "触碰智能体自身会话文件夹以外的文件。",
      "help.safety.database":
        "写入智能体的数据库。使用数据库的智能体只能通过其自带的命令行工具更改，没有该工具时为只读。",
      "help.safety.install":
        "安装或更新智能体的命令行工具。可在<strong>设置</strong>中选择你的工具所在位置。",
      "help.subagents.title": "子智能体",
      "help.subagents.body":
        "智能体自行启动的会话会嵌套显示在启动它们的会话下方；展开父会话即可看到。有些智能体会随父会话一起删除它们，确认窗口会说明这种情况。父会话已不存在的子智能体会显示<em>父会话已删除</em>。",
      "help.labels.title": "标签",
      "help.labels.body":
        "标签由 Marduk 而非智能体保存，因此归档后依然保留。在 Marduk 中删除会话会同时删除其标签。以其他方式删除的会话，其标签会在该会话消失满 7 天后删除。",
      "help.agents.loading": "正在加载…",
      "help.agents.none": "没有智能体。",
      "help.agents.list": "智能体",
      "help.agents.delete": "删除",
      "help.agents.archive": "归档",
      "help.agents.reads": "读取会话的位置",
      "help.done": "完成",
    },
    ru: {
      "help.title": "Как Marduk изменяет сессии",
      "help.topics": "Разделы справки",
      "help.tab.deleting": "Удаление",
      "help.tab.archiving": "Архивация",
      "help.tab.safety": "Безопасность",
      "help.tab.subagents": "Субагенты и метки",
      "help.tab.agents": "Агенты",
      "help.deleting.before.title": "Прежде чем что-то изменится",
      "help.deleting.before.body":
        "<strong>Удалить…</strong> показывает каждый файл, который будет перемещён, и каждую команду, которая будет выполнена. Ничего не меняется, пока вы не подтвердите.",
      "help.deleting.trash.title": "Корзина",
      "help.deleting.trash.body":
        "Файлы агентов без собственной команды удаления перемещаются в системную Корзину (в Windows — в Корзину), откуда их можно восстановить. Если файл нельзя переместить в Корзину, Marduk сообщает об ошибке, а не удаляет его безвозвратно.",
      "help.deleting.permanent.title": "Безвозвратно",
      "help.deleting.permanent.body":
        "Если у агента есть собственная команда удаления, Marduk запускает её, чтобы индексы агента оставались согласованными. Такое удаление нельзя отменить. В подтверждении оно помечено как <em>Безвозвратно</em>, и показана точная команда.",
      "help.deleting.seeAgents":
        "На вкладке <strong>Агенты</strong> указано, что относится к каждому агенту.",
      "help.archiving.agent.title": "Только через агента",
      "help.archiving.agent.body":
        "<strong>Архивировать</strong> предлагается только для агентов с собственной командой архивации и запускает эту команду. Сессия остаётся в агенте, и он по-прежнему может её открыть.",
      "help.archiving.none.title": "Никаких придуманных архивов",
      "help.archiving.none.body":
        "Marduk никогда не создаёт папки архива, о которых агент не знает. Для агентов без архива удалите сессию или оставьте её.",
      "help.safety.title": "Чего Marduk не делает",
      "help.safety.running":
        "Не изменяет сессию в состоянии <em>Выполняется</em>, то есть с записями за последние 5 минут.",
      "help.safety.changed":
        "Не изменяет сессию, которая изменилась после последнего сканирования. Пересканируйте и попробуйте снова.",
      "help.safety.outside":
        "Не трогает файлы за пределами папок сессий самого агента.",
      "help.safety.database":
        "Не пишет в базу данных агента. Агенты с базой данных изменяются только через собственный инструмент командной строки, а без него доступны только для чтения.",
      "help.safety.install":
        "Не устанавливает и не обновляет инструмент командной строки агента. Укажите, где находится ваш, в разделе <strong>Настройки</strong>.",
      "help.subagents.title": "Субагенты",
      "help.subagents.body":
        "Сессии, которые агент запустил сам, вложены в сессию, которая их запустила; разверните родительскую сессию, чтобы их увидеть. Некоторые агенты удаляют их вместе с родительской, и подтверждение сообщает об этом. У субагента, родительская сессия которого исчезла, отображается <em>Родительская сессия удалена</em>.",
      "help.labels.title": "Метки",
      "help.labels.body":
        "Метки хранит Marduk, а не агент, поэтому они сохраняются при архивации. Удаление сессии в Marduk удаляет её метки. Метки сессий, удалённых другим способом, удаляются, когда сессия отсутствует 7 дней.",
      "help.agents.loading": "Загрузка…",
      "help.agents.none": "Нет агентов.",
      "help.agents.list": "Агент",
      "help.agents.delete": "Удаление",
      "help.agents.archive": "Архивация",
      "help.agents.reads": "Читает сессии из",
      "help.done": "Готово",
    },
  },
);
