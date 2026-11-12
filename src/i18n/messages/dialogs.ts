import { defineMessages } from "../catalog";

/** The delete confirmation, the Files dialog and file lists. */
export const dialogs = defineMessages(
  {
    "dialogs.cancel": "Cancel",
    "dialogs.close": "Close",
    "dialogs.working": "Working…",
    "dialogs.delete.progress": "Delete progress",
    "dialogs.delete.titleTrash": {
      one: "Move {count} session to the Trash?",
      other: "Move {count} sessions to the Trash?",
    },
    "dialogs.delete.titleDelete": {
      one: "Delete {count} session?",
      other: "Delete {count} sessions?",
    },
    "dialogs.delete.trashNote": {
      one: "{count} session will be moved to the system Trash. You can restore them from there.",
      other:
        "{count} sessions will be moved to the system Trash. You can restore them from there.",
    },
    "dialogs.delete.permanentNote": {
      one: "{count} session will be {permanently} by the agent's own command and can't be restored.",
      other:
        "{count} sessions will be {permanently} by the agent's own command and can't be restored.",
    },
    "dialogs.delete.permanently": "permanently deleted",
    "dialogs.delete.permanentBadge": "Permanent",
    "dialogs.delete.memoryHeading": "Project memory",
    "dialogs.delete.memoryOffer": {
      one: "Also move {agent}'s memory for {path} to the Trash ({count} file)",
      other:
        "Also move {agent}'s memory for {path} to the Trash ({count} files)",
    },
    "dialogs.delete.memoryProjectGone":
      "Its project folder no longer exists, and this deletes the project's last session.",
    "dialogs.delete.memoryProjectExists":
      "This deletes the project's last session, but {agent} would still use this memory the next time you work there.",
    "dialogs.delete.purgeHeading": "Project data",
    "dialogs.delete.purgeOffer": "Also purge {agent}'s data for {path}",
    "dialogs.delete.purgeNote":
      "Runs after the sessions are deleted, and only if none are left in the project. It permanently deletes the project's memory, its prompts in {agent}'s prompt history and its settings entry (trust, allowed tools, MCP servers).",
    "dialogs.delete.skipped": "Skipped ({count})",
    "dialogs.delete.confirmDelete": "Delete",
    "dialogs.delete.confirmTrash": "Move to Trash",
    "dialogs.files.databaseNote":
      "{agent} keeps all sessions in one shared database, so this session has no files of its own. Deleting it removes its rows through {agent}'s own command; the database file stays.",
    "dialogs.files.missing": "missing",
    "dialogs.files.showInFolder": "Show in folder",
  },
  {
    es: {
      "dialogs.cancel": "Cancelar",
      "dialogs.close": "Cerrar",
      "dialogs.working": "Procesando…",
      "dialogs.delete.progress": "Progreso de la eliminación",
      "dialogs.delete.titleTrash": {
        one: "¿Mover {count} sesión a la Papelera?",
        other: "¿Mover {count} sesiones a la Papelera?",
      },
      "dialogs.delete.titleDelete": {
        one: "¿Eliminar {count} sesión?",
        other: "¿Eliminar {count} sesiones?",
      },
      "dialogs.delete.trashNote": {
        one: "{count} sesión se moverá a la Papelera del sistema. Puedes restaurarla desde allí.",
        other:
          "{count} sesiones se moverán a la Papelera del sistema. Puedes restaurarlas desde allí.",
      },
      "dialogs.delete.permanentNote": {
        one: "{count} sesión se va a {permanently} con el propio comando del agente y no se podrá restaurar.",
        other:
          "{count} sesiones se van a {permanently} con el propio comando del agente y no se podrán restaurar.",
      },
      "dialogs.delete.permanently": "eliminar permanentemente",
      "dialogs.delete.permanentBadge": "Permanente",
      "dialogs.delete.memoryHeading": "Memoria del proyecto",
      "dialogs.delete.memoryOffer": {
        one: "Mover también la memoria de {agent} para {path} a la Papelera ({count} archivo)",
        other:
          "Mover también la memoria de {agent} para {path} a la Papelera ({count} archivos)",
      },
      "dialogs.delete.memoryProjectGone":
        "La carpeta del proyecto ya no existe, y esto elimina la última sesión del proyecto.",
      "dialogs.delete.memoryProjectExists":
        "Esto elimina la última sesión del proyecto, pero {agent} seguiría usando esta memoria la próxima vez que trabajes allí.",
      "dialogs.delete.purgeHeading": "Datos del proyecto",
      "dialogs.delete.purgeOffer":
        "Purgar también los datos de {agent} para {path}",
      "dialogs.delete.purgeNote":
        "Se ejecuta después de eliminar las sesiones y solo si no queda ninguna en el proyecto. Elimina de forma permanente la memoria del proyecto, sus prompts en el historial de {agent} y su entrada de configuración (confianza, herramientas permitidas, servidores MCP).",
      "dialogs.delete.skipped": "Omitidas ({count})",
      "dialogs.delete.confirmDelete": "Eliminar",
      "dialogs.delete.confirmTrash": "Mover a la Papelera",
      "dialogs.files.databaseNote":
        "{agent} guarda todas las sesiones en una base de datos compartida, así que esta sesión no tiene archivos propios. Al eliminarla, sus filas se borran con el propio comando de {agent}; el archivo de la base de datos se conserva.",
      "dialogs.files.missing": "no existe",
      "dialogs.files.showInFolder": "Mostrar en la carpeta",
    },
    "pt-BR": {
      "dialogs.cancel": "Cancelar",
      "dialogs.close": "Fechar",
      "dialogs.working": "Processando…",
      "dialogs.delete.progress": "Progresso da exclusão",
      "dialogs.delete.titleTrash": {
        one: "Mover {count} sessão para a Lixeira?",
        other: "Mover {count} sessões para a Lixeira?",
      },
      "dialogs.delete.titleDelete": {
        one: "Excluir {count} sessão?",
        other: "Excluir {count} sessões?",
      },
      "dialogs.delete.trashNote": {
        one: "{count} sessão será movida para a Lixeira do sistema. Você pode restaurá-la de lá.",
        other:
          "{count} sessões serão movidas para a Lixeira do sistema. Você pode restaurá-las de lá.",
      },
      "dialogs.delete.permanentNote": {
        one: "{count} sessão será {permanently} pelo próprio comando do agente e não poderá ser restaurada.",
        other:
          "{count} sessões serão {permanently} pelo próprio comando do agente e não poderão ser restauradas.",
      },
      "dialogs.delete.permanently": "excluída permanentemente",
      "dialogs.delete.permanentBadge": "Permanente",
      "dialogs.delete.memoryHeading": "Memória do projeto",
      "dialogs.delete.memoryOffer": {
        one: "Mover também a memória do {agent} para {path} para a Lixeira ({count} arquivo)",
        other:
          "Mover também a memória do {agent} para {path} para a Lixeira ({count} arquivos)",
      },
      "dialogs.delete.memoryProjectGone":
        "A pasta do projeto não existe mais, e isto exclui a última sessão do projeto.",
      "dialogs.delete.memoryProjectExists":
        "Isto exclui a última sessão do projeto, mas o {agent} ainda usaria esta memória na próxima vez que você trabalhar nele.",
      "dialogs.delete.purgeHeading": "Dados do projeto",
      "dialogs.delete.purgeOffer":
        "Também limpar os dados do {agent} para {path}",
      "dialogs.delete.purgeNote":
        "É executado depois que as sessões são excluídas e só se nenhuma restar no projeto. Exclui permanentemente a memória do projeto, seus prompts no histórico do {agent} e sua entrada nas configurações (confiança, ferramentas permitidas, servidores MCP).",
      "dialogs.delete.skipped": "Ignoradas ({count})",
      "dialogs.delete.confirmDelete": "Excluir",
      "dialogs.delete.confirmTrash": "Mover para a Lixeira",
      "dialogs.files.databaseNote":
        "O {agent} guarda todas as sessões em um único banco de dados compartilhado, então esta sessão não tem arquivos próprios. Excluí-la remove as linhas dela pelo próprio comando do {agent}; o arquivo do banco de dados continua.",
      "dialogs.files.missing": "ausente",
      "dialogs.files.showInFolder": "Mostrar na pasta",
    },
    fr: {
      "dialogs.cancel": "Annuler",
      "dialogs.close": "Fermer",
      "dialogs.working": "En cours…",
      "dialogs.delete.progress": "Progression de la suppression",
      "dialogs.delete.titleTrash": {
        one: "Placer {count} session dans la Corbeille ?",
        other: "Placer {count} sessions dans la Corbeille ?",
      },
      "dialogs.delete.titleDelete": {
        one: "Supprimer {count} session ?",
        other: "Supprimer {count} sessions ?",
      },
      "dialogs.delete.trashNote": {
        one: "{count} session sera placée dans la Corbeille du système. Vous pourrez l'y restaurer.",
        other:
          "{count} sessions seront placées dans la Corbeille du système. Vous pourrez les y restaurer.",
      },
      "dialogs.delete.permanentNote": {
        one: "{count} session sera {permanently} par la commande de l'agent lui-même et ne pourra pas être restaurée.",
        other:
          "{count} sessions seront {permanently} par la commande de l'agent lui-même et ne pourront pas être restaurées.",
      },
      "dialogs.delete.permanently": "supprimée définitivement",
      "dialogs.delete.permanentBadge": "Définitive",
      "dialogs.delete.memoryHeading": "Mémoire du projet",
      "dialogs.delete.memoryOffer": {
        one: "Placer aussi la mémoire de {agent} pour {path} dans la Corbeille ({count} fichier)",
        other:
          "Placer aussi la mémoire de {agent} pour {path} dans la Corbeille ({count} fichiers)",
      },
      "dialogs.delete.memoryProjectGone":
        "Le dossier du projet n'existe plus, et ceci supprime la dernière session du projet.",
      "dialogs.delete.memoryProjectExists":
        "Ceci supprime la dernière session du projet, mais {agent} utiliserait encore cette mémoire la prochaine fois que vous y travaillerez.",
      "dialogs.delete.purgeHeading": "Données du projet",
      "dialogs.delete.purgeOffer":
        "Purger aussi les données de {agent} pour {path}",
      "dialogs.delete.purgeNote":
        "S'exécute après la suppression des sessions, et seulement s'il n'en reste aucune dans le projet. Supprime définitivement la mémoire du projet, ses prompts dans l'historique de {agent} et son entrée de configuration (confiance, outils autorisés, serveurs MCP).",
      "dialogs.delete.skipped": "Ignorées ({count})",
      "dialogs.delete.confirmDelete": "Supprimer",
      "dialogs.delete.confirmTrash": "Placer dans la Corbeille",
      "dialogs.files.databaseNote":
        "{agent} conserve toutes les sessions dans une seule base de données partagée : cette session n'a donc pas de fichiers propres. La supprimer efface ses lignes via la commande de {agent} lui-même ; le fichier de la base de données reste.",
      "dialogs.files.missing": "introuvable",
      "dialogs.files.showInFolder": "Afficher dans le dossier",
    },
    de: {
      "dialogs.cancel": "Abbrechen",
      "dialogs.close": "Schließen",
      "dialogs.working": "Wird ausgeführt …",
      "dialogs.delete.progress": "Löschfortschritt",
      "dialogs.delete.titleTrash": {
        one: "{count} Sitzung in den Papierkorb legen?",
        other: "{count} Sitzungen in den Papierkorb legen?",
      },
      "dialogs.delete.titleDelete": {
        one: "{count} Sitzung löschen?",
        other: "{count} Sitzungen löschen?",
      },
      "dialogs.delete.trashNote": {
        one: "{count} Sitzung wird in den Papierkorb des Systems gelegt. Dort können Sie sie wiederherstellen.",
        other:
          "{count} Sitzungen werden in den Papierkorb des Systems gelegt. Dort können Sie sie wiederherstellen.",
      },
      "dialogs.delete.permanentNote": {
        one: "{count} Sitzung wird mit dem eigenen Befehl des Agenten {permanently} und kann nicht wiederhergestellt werden.",
        other:
          "{count} Sitzungen werden mit dem eigenen Befehl des Agenten {permanently} und können nicht wiederhergestellt werden.",
      },
      "dialogs.delete.permanently": "endgültig gelöscht",
      "dialogs.delete.permanentBadge": "Endgültig",
      "dialogs.delete.memoryHeading": "Projektgedächtnis",
      "dialogs.delete.memoryOffer": {
        one: "Auch das Gedächtnis von {agent} für {path} in den Papierkorb legen ({count} Datei)",
        other:
          "Auch das Gedächtnis von {agent} für {path} in den Papierkorb legen ({count} Dateien)",
      },
      "dialogs.delete.memoryProjectGone":
        "Der Projektordner existiert nicht mehr, und damit wird die letzte Sitzung des Projekts gelöscht.",
      "dialogs.delete.memoryProjectExists":
        "Damit wird die letzte Sitzung des Projekts gelöscht, aber {agent} würde dieses Gedächtnis beim nächsten Mal, wenn Sie dort arbeiten, weiterhin verwenden.",
      "dialogs.delete.purgeHeading": "Projektdaten",
      "dialogs.delete.purgeOffer":
        "Auch die Daten von {agent} für {path} bereinigen",
      "dialogs.delete.purgeNote":
        "Läuft, nachdem die Sitzungen gelöscht wurden, und nur, wenn im Projekt keine mehr übrig ist. Löscht dauerhaft das Gedächtnis des Projekts, seine Prompts im Verlauf von {agent} und seinen Eintrag in den Einstellungen (Vertrauen, erlaubte Tools, MCP-Server).",
      "dialogs.delete.skipped": "Übersprungen ({count})",
      "dialogs.delete.confirmDelete": "Löschen",
      "dialogs.delete.confirmTrash": "In den Papierkorb legen",
      "dialogs.files.databaseNote":
        "{agent} speichert alle Sitzungen in einer gemeinsamen Datenbank, daher hat diese Sitzung keine eigenen Dateien. Beim Löschen werden ihre Zeilen mit dem eigenen Befehl von {agent} entfernt; die Datenbankdatei bleibt erhalten.",
      "dialogs.files.missing": "fehlt",
      "dialogs.files.showInFolder": "Im Ordner anzeigen",
    },
    ja: {
      "dialogs.cancel": "キャンセル",
      "dialogs.close": "閉じる",
      "dialogs.working": "処理中…",
      "dialogs.delete.progress": "削除の進行状況",
      "dialogs.delete.titleTrash": {
        other: "{count} 件のセッションをゴミ箱に移動しますか？",
      },
      "dialogs.delete.titleDelete": {
        other: "{count} 件のセッションを削除しますか？",
      },
      "dialogs.delete.trashNote": {
        other:
          "{count} 件のセッションをシステムのゴミ箱に移動します。ゴミ箱から元に戻せます。",
      },
      "dialogs.delete.permanentNote": {
        other:
          "{count} 件のセッションはエージェント自身のコマンドで{permanently}され、元に戻せません。",
      },
      "dialogs.delete.permanently": "完全に削除",
      "dialogs.delete.permanentBadge": "完全削除",
      "dialogs.delete.memoryHeading": "プロジェクトのメモリ",
      "dialogs.delete.memoryOffer": {
        other:
          "{path} の {agent} のメモリもゴミ箱に移動する（{count} ファイル）",
      },
      "dialogs.delete.memoryProjectGone":
        "プロジェクトフォルダはもう存在せず、この操作でプロジェクトの最後のセッションが削除されます。",
      "dialogs.delete.memoryProjectExists":
        "この操作でプロジェクトの最後のセッションが削除されますが、次にそこで作業するときも {agent} はこのメモリを使います。",
      "dialogs.delete.purgeHeading": "プロジェクトのデータ",
      "dialogs.delete.purgeOffer": "{path} の {agent} のデータも消去する",
      "dialogs.delete.purgeNote":
        "セッションの削除後に、プロジェクトにセッションが残っていない場合だけ実行されます。プロジェクトのメモリ、{agent} のプロンプト履歴にあるそのプロジェクトのプロンプト、設定のエントリ（信頼、許可したツール、MCP サーバー）を完全に削除します。",
      "dialogs.delete.skipped": "スキップ（{count}）",
      "dialogs.delete.confirmDelete": "削除",
      "dialogs.delete.confirmTrash": "ゴミ箱に移動",
      "dialogs.files.databaseNote":
        "{agent} はすべてのセッションを 1 つの共有データベースに保存するため、このセッション自体のファイルはありません。削除すると {agent} 自身のコマンドでその行が削除されます。データベースファイルは残ります。",
      "dialogs.files.missing": "見つかりません",
      "dialogs.files.showInFolder": "フォルダで表示",
    },
    ko: {
      "dialogs.cancel": "취소",
      "dialogs.close": "닫기",
      "dialogs.working": "처리 중…",
      "dialogs.delete.progress": "삭제 진행률",
      "dialogs.delete.titleTrash": {
        other: "세션 {count}개를 휴지통으로 이동할까요?",
      },
      "dialogs.delete.titleDelete": {
        other: "세션 {count}개를 삭제할까요?",
      },
      "dialogs.delete.trashNote": {
        other:
          "세션 {count}개가 시스템 휴지통으로 이동됩니다. 휴지통에서 복원할 수 있습니다.",
      },
      "dialogs.delete.permanentNote": {
        other:
          "세션 {count}개가 에이전트 자체 명령으로 {permanently}되며 복원할 수 없습니다.",
      },
      "dialogs.delete.permanently": "영구 삭제",
      "dialogs.delete.permanentBadge": "영구",
      "dialogs.delete.memoryHeading": "프로젝트 메모리",
      "dialogs.delete.memoryOffer": {
        other:
          "{path}에 대한 {agent} 메모리도 휴지통으로 이동 (파일 {count}개)",
      },
      "dialogs.delete.memoryProjectGone":
        "프로젝트 폴더가 더 이상 없으며, 이 작업으로 프로젝트의 마지막 세션이 삭제됩니다.",
      "dialogs.delete.memoryProjectExists":
        "이 작업으로 프로젝트의 마지막 세션이 삭제되지만, 다음에 그 프로젝트에서 작업할 때 {agent}는 이 메모리를 계속 사용합니다.",
      "dialogs.delete.purgeHeading": "프로젝트 데이터",
      "dialogs.delete.purgeOffer": "{path}에 대한 {agent} 데이터도 정리",
      "dialogs.delete.purgeNote":
        "세션을 삭제한 뒤, 프로젝트에 남은 세션이 없을 때만 실행됩니다. 프로젝트의 메모리, {agent} 프롬프트 기록에 있는 프로젝트의 프롬프트, 설정 항목(신뢰, 허용된 도구, MCP 서버)을 영구적으로 삭제합니다.",
      "dialogs.delete.skipped": "건너뜀 ({count})",
      "dialogs.delete.confirmDelete": "삭제",
      "dialogs.delete.confirmTrash": "휴지통으로 이동",
      "dialogs.files.databaseNote":
        "{agent}는 모든 세션을 하나의 공유 데이터베이스에 보관하므로 이 세션에는 자체 파일이 없습니다. 삭제하면 {agent} 자체 명령으로 해당 행이 제거되며, 데이터베이스 파일은 남습니다.",
      "dialogs.files.missing": "없음",
      "dialogs.files.showInFolder": "폴더에서 보기",
    },
    "zh-Hans": {
      "dialogs.cancel": "取消",
      "dialogs.close": "关闭",
      "dialogs.working": "正在处理…",
      "dialogs.delete.progress": "删除进度",
      "dialogs.delete.titleTrash": {
        other: "将 {count} 个会话移到废纸篓？",
      },
      "dialogs.delete.titleDelete": {
        other: "删除 {count} 个会话？",
      },
      "dialogs.delete.trashNote": {
        other: "{count} 个会话将被移到系统废纸篓，你可以从那里恢复。",
      },
      "dialogs.delete.permanentNote": {
        other: "{count} 个会话将由智能体自己的命令{permanently}，无法恢复。",
      },
      "dialogs.delete.permanently": "永久删除",
      "dialogs.delete.permanentBadge": "永久",
      "dialogs.delete.memoryHeading": "项目记忆",
      "dialogs.delete.memoryOffer": {
        other: "同时将 {agent} 在 {path} 的记忆移到废纸篓（{count} 个文件）",
      },
      "dialogs.delete.memoryProjectGone":
        "项目文件夹已不存在，此操作将删除该项目的最后一个会话。",
      "dialogs.delete.memoryProjectExists":
        "此操作将删除该项目的最后一个会话，但下次你在那里工作时，{agent} 仍会使用这些记忆。",
      "dialogs.delete.purgeHeading": "项目数据",
      "dialogs.delete.purgeOffer": "同时清除 {agent} 中 {path} 的数据",
      "dialogs.delete.purgeNote":
        "在会话删除后运行，且仅在项目中没有剩余会话时运行。它会永久删除项目的记忆、{agent} 提示历史中该项目的提示，以及它的设置条目（信任、允许的工具、MCP 服务器）。",
      "dialogs.delete.skipped": "已跳过（{count}）",
      "dialogs.delete.confirmDelete": "删除",
      "dialogs.delete.confirmTrash": "移到废纸篓",
      "dialogs.files.databaseNote":
        "{agent} 将所有会话保存在一个共享数据库中，因此此会话没有自己的文件。删除它会通过 {agent} 自己的命令移除其数据行；数据库文件会保留。",
      "dialogs.files.missing": "缺失",
      "dialogs.files.showInFolder": "在文件夹中显示",
    },
    ru: {
      "dialogs.cancel": "Отмена",
      "dialogs.close": "Закрыть",
      "dialogs.working": "Выполняется…",
      "dialogs.delete.progress": "Ход удаления",
      "dialogs.delete.titleTrash": {
        one: "Переместить {count} сессию в Корзину?",
        few: "Переместить {count} сессии в Корзину?",
        many: "Переместить {count} сессий в Корзину?",
        other: "Переместить {count} сессии в Корзину?",
      },
      "dialogs.delete.titleDelete": {
        one: "Удалить {count} сессию?",
        few: "Удалить {count} сессии?",
        many: "Удалить {count} сессий?",
        other: "Удалить {count} сессии?",
      },
      "dialogs.delete.trashNote": {
        one: "{count} сессия будет перемещена в системную Корзину. Оттуда её можно восстановить.",
        few: "{count} сессии будут перемещены в системную Корзину. Оттуда их можно восстановить.",
        many: "{count} сессий будут перемещены в системную Корзину. Оттуда их можно восстановить.",
        other:
          "{count} сессии будут перемещены в системную Корзину. Оттуда их можно восстановить.",
      },
      "dialogs.delete.permanentNote": {
        one: "{count} сессия будет {permanently} собственной командой агента, и восстановить её будет нельзя.",
        few: "{count} сессии будут {permanently} собственной командой агента, и восстановить их будет нельзя.",
        many: "{count} сессий будут {permanently} собственной командой агента, и восстановить их будет нельзя.",
        other:
          "{count} сессии будут {permanently} собственной командой агента, и восстановить их будет нельзя.",
      },
      "dialogs.delete.permanently": "удалены безвозвратно",
      "dialogs.delete.permanentBadge": "Безвозвратно",
      "dialogs.delete.memoryHeading": "Память проекта",
      "dialogs.delete.memoryOffer": {
        one: "Также переместить память {agent} для {path} в Корзину ({count} файл)",
        few: "Также переместить память {agent} для {path} в Корзину ({count} файла)",
        many: "Также переместить память {agent} для {path} в Корзину ({count} файлов)",
        other:
          "Также переместить память {agent} для {path} в Корзину ({count} файла)",
      },
      "dialogs.delete.memoryProjectGone":
        "Папки проекта больше нет, и это удаляет последнюю сессию проекта.",
      "dialogs.delete.memoryProjectExists":
        "Это удаляет последнюю сессию проекта, но {agent} продолжит использовать эту память, когда вы снова будете там работать.",
      "dialogs.delete.purgeHeading": "Данные проекта",
      "dialogs.delete.purgeOffer": "Также очистить данные {agent} для {path}",
      "dialogs.delete.purgeNote":
        "Выполняется после удаления сессий и только если в проекте не осталось ни одной. Безвозвратно удаляет память проекта, его запросы в истории {agent} и его запись в настройках (доверие, разрешённые инструменты, MCP-серверы).",
      "dialogs.delete.skipped": "Пропущено ({count})",
      "dialogs.delete.confirmDelete": "Удалить",
      "dialogs.delete.confirmTrash": "Переместить в Корзину",
      "dialogs.files.databaseNote":
        "{agent} хранит все сессии в одной общей базе данных, поэтому у этой сессии нет собственных файлов. При удалении её строки удаляются собственной командой {agent}; файл базы данных остаётся.",
      "dialogs.files.missing": "отсутствует",
      "dialogs.files.showInFolder": "Показать в папке",
    },
  },
);
