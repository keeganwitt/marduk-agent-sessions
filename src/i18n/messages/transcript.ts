import { defineMessages } from "../catalog";

/** The transcript side panel and Markdown rendering. */
export const transcript = defineMessages(
  {
    "transcript.role.user": "You",
    "transcript.role.assistant": "Assistant",
    "transcript.role.tool": "Tool",
    "transcript.role.context": "Injected context",
    "transcript.output": "Output",
    "transcript.close": "Close",
    "transcript.closeLabel": "Close transcript",
    "transcript.toolCalls": "Tool calls",
    "transcript.jumpLabel": "Jump to prompt",
    "transcript.jumpCount": "Jump to prompt ({count})",
    "transcript.loading": "Loading…",
    "transcript.noMessages": "This session has no messages to show.",
    "transcript.allHidden":
      "Everything in this session is hidden by the filters above. Turn on Tool calls or Injected context to see it.",
    "transcript.running":
      "This session is still running, so the transcript may be incomplete.",
    "transcript.shortened": "Some long messages or tool outputs are shortened.",
    "transcript.omitted": {
      one: "{count} message in the middle of this session isn't shown. Use Files to open the full transcript.",
      other:
        "{count} messages in the middle of this session aren't shown. Use Files to open the full transcript.",
    },
    "transcript.image": "[image]",
    "transcript.imageAlt": "[image: {alt}]",
  },
  {
    es: {
      "transcript.role.user": "Tú",
      "transcript.role.assistant": "Asistente",
      "transcript.role.tool": "Herramienta",
      "transcript.role.context": "Contexto inyectado",
      "transcript.output": "Salida",
      "transcript.close": "Cerrar",
      "transcript.closeLabel": "Cerrar transcripción",
      "transcript.toolCalls": "Llamadas a herramientas",
      "transcript.jumpLabel": "Ir a una instrucción",
      "transcript.jumpCount": "Ir a una instrucción ({count})",
      "transcript.loading": "Cargando…",
      "transcript.noMessages": "Esta sesión no tiene mensajes que mostrar.",
      "transcript.allHidden":
        "Los filtros de arriba ocultan todo el contenido de esta sesión. Activa Llamadas a herramientas o Contexto inyectado para verlo.",
      "transcript.running":
        "Esta sesión sigue en ejecución, así que la transcripción puede estar incompleta.",
      "transcript.shortened":
        "Algunos mensajes o resultados de herramientas largos están acortados.",
      "transcript.omitted": {
        one: "No se muestra {count} mensaje de la mitad de esta sesión. Usa Archivos para abrir la transcripción completa.",
        other:
          "No se muestran {count} mensajes de la mitad de esta sesión. Usa Archivos para abrir la transcripción completa.",
      },
      "transcript.image": "[imagen]",
      "transcript.imageAlt": "[imagen: {alt}]",
    },
    "pt-BR": {
      "transcript.role.user": "Você",
      "transcript.role.assistant": "Assistente",
      "transcript.role.tool": "Ferramenta",
      "transcript.role.context": "Contexto injetado",
      "transcript.output": "Saída",
      "transcript.close": "Fechar",
      "transcript.closeLabel": "Fechar transcrição",
      "transcript.toolCalls": "Chamadas de ferramentas",
      "transcript.jumpLabel": "Ir para um prompt",
      "transcript.jumpCount": "Ir para um prompt ({count})",
      "transcript.loading": "Carregando…",
      "transcript.noMessages": "Esta sessão não tem mensagens para mostrar.",
      "transcript.allHidden":
        "Os filtros acima ocultam tudo nesta sessão. Ative Chamadas de ferramentas ou Contexto injetado para ver.",
      "transcript.running":
        "Esta sessão ainda está em execução, então a transcrição pode estar incompleta.",
      "transcript.shortened":
        "Algumas mensagens ou saídas de ferramentas longas foram encurtadas.",
      "transcript.omitted": {
        one: "{count} mensagem do meio desta sessão não é mostrada. Use Arquivos para abrir a transcrição completa.",
        other:
          "{count} mensagens do meio desta sessão não são mostradas. Use Arquivos para abrir a transcrição completa.",
      },
      "transcript.image": "[imagem]",
      "transcript.imageAlt": "[imagem: {alt}]",
    },
    fr: {
      "transcript.role.user": "Vous",
      "transcript.role.assistant": "Assistant",
      "transcript.role.tool": "Outil",
      "transcript.role.context": "Contexte injecté",
      "transcript.output": "Sortie",
      "transcript.close": "Fermer",
      "transcript.closeLabel": "Fermer la transcription",
      "transcript.toolCalls": "Appels d'outils",
      "transcript.jumpLabel": "Aller à une requête",
      "transcript.jumpCount": "Aller à une requête ({count})",
      "transcript.loading": "Chargement…",
      "transcript.noMessages": "Cette session n'a aucun message à afficher.",
      "transcript.allHidden":
        "Les filtres ci-dessus masquent tout le contenu de cette session. Activez Appels d'outils ou Contexte injecté pour l'afficher.",
      "transcript.running":
        "Cette session est toujours en cours : la transcription peut être incomplète.",
      "transcript.shortened":
        "Certains messages ou sorties d'outils longs sont raccourcis.",
      "transcript.omitted": {
        one: "{count} message au milieu de cette session n'est pas affiché. Utilisez Fichiers pour ouvrir la transcription complète.",
        other:
          "{count} messages au milieu de cette session ne sont pas affichés. Utilisez Fichiers pour ouvrir la transcription complète.",
      },
      "transcript.image": "[image]",
      "transcript.imageAlt": "[image : {alt}]",
    },
    de: {
      "transcript.role.user": "Sie",
      "transcript.role.assistant": "Assistent",
      "transcript.role.tool": "Werkzeug",
      "transcript.role.context": "Eingefügter Kontext",
      "transcript.output": "Ausgabe",
      "transcript.close": "Schließen",
      "transcript.closeLabel": "Verlauf schließen",
      "transcript.toolCalls": "Werkzeugaufrufe",
      "transcript.jumpLabel": "Zu Eingabe springen",
      "transcript.jumpCount": "Zu Eingabe springen ({count})",
      "transcript.loading": "Wird geladen …",
      "transcript.noMessages":
        "Diese Sitzung enthält keine anzeigbaren Nachrichten.",
      "transcript.allHidden":
        "Die Filter oben blenden alles in dieser Sitzung aus. Aktivieren Sie Werkzeugaufrufe oder Eingefügter Kontext, um es zu sehen.",
      "transcript.running":
        "Diese Sitzung läuft noch, daher kann der Verlauf unvollständig sein.",
      "transcript.shortened":
        "Einige lange Nachrichten oder Werkzeugausgaben sind gekürzt.",
      "transcript.omitted": {
        one: "{count} Nachricht in der Mitte dieser Sitzung wird nicht angezeigt. Öffnen Sie über Dateien den vollständigen Verlauf.",
        other:
          "{count} Nachrichten in der Mitte dieser Sitzung werden nicht angezeigt. Öffnen Sie über Dateien den vollständigen Verlauf.",
      },
      "transcript.image": "[Bild]",
      "transcript.imageAlt": "[Bild: {alt}]",
    },
    ja: {
      "transcript.role.user": "あなた",
      "transcript.role.assistant": "アシスタント",
      "transcript.role.tool": "ツール",
      "transcript.role.context": "挿入されたコンテキスト",
      "transcript.output": "出力",
      "transcript.close": "閉じる",
      "transcript.closeLabel": "トランスクリプトを閉じる",
      "transcript.toolCalls": "ツール呼び出し",
      "transcript.jumpLabel": "プロンプトへ移動",
      "transcript.jumpCount": "プロンプトへ移動（{count}）",
      "transcript.loading": "読み込み中…",
      "transcript.noMessages":
        "このセッションには表示できるメッセージがありません。",
      "transcript.allHidden":
        "上のフィルタでこのセッションの内容がすべて非表示になっています。表示するには「ツール呼び出し」または「挿入されたコンテキスト」をオンにしてください。",
      "transcript.running":
        "このセッションはまだ実行中のため、トランスクリプトが不完全な場合があります。",
      "transcript.shortened":
        "長いメッセージやツール出力の一部は短縮されています。",
      "transcript.omitted": {
        other:
          "このセッションの途中の {count} 件のメッセージは表示されていません。完全なトランスクリプトは「ファイル」から開いてください。",
      },
      "transcript.image": "[画像]",
      "transcript.imageAlt": "[画像: {alt}]",
    },
    ko: {
      "transcript.role.user": "나",
      "transcript.role.assistant": "어시스턴트",
      "transcript.role.tool": "도구",
      "transcript.role.context": "삽입된 컨텍스트",
      "transcript.output": "출력",
      "transcript.close": "닫기",
      "transcript.closeLabel": "대화 기록 닫기",
      "transcript.toolCalls": "도구 호출",
      "transcript.jumpLabel": "프롬프트로 이동",
      "transcript.jumpCount": "프롬프트로 이동 ({count})",
      "transcript.loading": "불러오는 중…",
      "transcript.noMessages": "이 세션에는 표시할 메시지가 없습니다.",
      "transcript.allHidden":
        "위의 필터 때문에 이 세션의 모든 내용이 숨겨져 있습니다. 보려면 도구 호출 또는 삽입된 컨텍스트를 켜세요.",
      "transcript.running":
        "이 세션은 아직 실행 중이므로 대화 기록이 불완전할 수 있습니다.",
      "transcript.shortened": "일부 긴 메시지나 도구 출력은 줄여서 표시됩니다.",
      "transcript.omitted": {
        other:
          "이 세션 중간의 메시지 {count}개는 표시되지 않습니다. 전체 대화 기록은 파일에서 여세요.",
      },
      "transcript.image": "[이미지]",
      "transcript.imageAlt": "[이미지: {alt}]",
    },
    "zh-Hans": {
      "transcript.role.user": "你",
      "transcript.role.assistant": "助手",
      "transcript.role.tool": "工具",
      "transcript.role.context": "注入的上下文",
      "transcript.output": "输出",
      "transcript.close": "关闭",
      "transcript.closeLabel": "关闭对话记录",
      "transcript.toolCalls": "工具调用",
      "transcript.jumpLabel": "跳转到提示",
      "transcript.jumpCount": "跳转到提示（{count}）",
      "transcript.loading": "正在加载…",
      "transcript.noMessages": "此会话没有可显示的消息。",
      "transcript.allHidden":
        "上方的筛选隐藏了此会话的全部内容。打开“工具调用”或“注入的上下文”即可查看。",
      "transcript.running": "此会话仍在运行，对话记录可能不完整。",
      "transcript.shortened": "部分较长的消息或工具输出已被截短。",
      "transcript.omitted": {
        other:
          "此会话中间的 {count} 条消息未显示。请通过“文件”打开完整的对话记录。",
      },
      "transcript.image": "[图片]",
      "transcript.imageAlt": "[图片：{alt}]",
    },
    ru: {
      "transcript.role.user": "Вы",
      "transcript.role.assistant": "Ассистент",
      "transcript.role.tool": "Инструмент",
      "transcript.role.context": "Внедрённый контекст",
      "transcript.output": "Вывод",
      "transcript.close": "Закрыть",
      "transcript.closeLabel": "Закрыть журнал",
      "transcript.toolCalls": "Вызовы инструментов",
      "transcript.jumpLabel": "Перейти к запросу",
      "transcript.jumpCount": "Перейти к запросу ({count})",
      "transcript.loading": "Загрузка…",
      "transcript.noMessages": "В этой сессии нет сообщений для показа.",
      "transcript.allHidden":
        "Фильтры выше скрывают всё в этой сессии. Включите «Вызовы инструментов» или «Внедрённый контекст», чтобы увидеть.",
      "transcript.running":
        "Эта сессия ещё выполняется, поэтому журнал может быть неполным.",
      "transcript.shortened":
        "Некоторые длинные сообщения или выводы инструментов сокращены.",
      "transcript.omitted": {
        one: "{count} сообщение в середине этой сессии не показано. Откройте полный журнал через «Файлы».",
        few: "{count} сообщения в середине этой сессии не показаны. Откройте полный журнал через «Файлы».",
        many: "{count} сообщений в середине этой сессии не показаны. Откройте полный журнал через «Файлы».",
        other:
          "{count} сообщения в середине этой сессии не показаны. Откройте полный журнал через «Файлы».",
      },
      "transcript.image": "[изображение]",
      "transcript.imageAlt": "[изображение: {alt}]",
    },
  },
);
