export interface EdgeVoice {
  id: string;
  lang: string;
  label: string;
  gender: '女声' | '男声';
}

// Curated Microsoft Edge Read Aloud voices. The online provider is opt-in.
export const EDGE_VOICES: EdgeVoice[] = [
  { id: 'zh-CN-XiaoxiaoNeural', lang: 'zh', label: '晓晓 · 温暖', gender: '女声' },
  { id: 'zh-CN-XiaoyiNeural', lang: 'zh', label: '晓伊 · 轻快', gender: '女声' },
  { id: 'zh-CN-YunxiNeural', lang: 'zh', label: '云希 · 年轻', gender: '男声' },
  { id: 'zh-CN-YunjianNeural', lang: 'zh', label: '云健 · 沉稳', gender: '男声' },
  { id: 'zh-CN-YunyangNeural', lang: 'zh', label: '云扬 · 播报', gender: '男声' },

  { id: 'en-US-AriaNeural', lang: 'en', label: 'Aria · 自然', gender: '女声' },
  { id: 'en-US-JennyNeural', lang: 'en', label: 'Jenny · 亲切', gender: '女声' },
  { id: 'en-US-GuyNeural', lang: 'en', label: 'Guy · 沉稳', gender: '男声' },
  { id: 'en-GB-SoniaNeural', lang: 'en', label: 'Sonia · 英式', gender: '女声' },
  { id: 'en-GB-RyanNeural', lang: 'en', label: 'Ryan · 英式', gender: '男声' },

  { id: 'ja-JP-NanamiNeural', lang: 'ja', label: '七海 · 自然', gender: '女声' },
  { id: 'ja-JP-KeitaNeural', lang: 'ja', label: '圭太 · 沉稳', gender: '男声' },
  { id: 'ko-KR-SunHiNeural', lang: 'ko', label: 'SunHi · 明亮', gender: '女声' },
  { id: 'ko-KR-InJoonNeural', lang: 'ko', label: 'InJoon · 沉稳', gender: '男声' },
  { id: 'fr-FR-DeniseNeural', lang: 'fr', label: 'Denise · 自然', gender: '女声' },
  { id: 'fr-FR-HenriNeural', lang: 'fr', label: 'Henri · 沉稳', gender: '男声' },
  { id: 'de-DE-KatjaNeural', lang: 'de', label: 'Katja · 自然', gender: '女声' },
  { id: 'de-DE-ConradNeural', lang: 'de', label: 'Conrad · 沉稳', gender: '男声' },
  { id: 'es-ES-ElviraNeural', lang: 'es', label: 'Elvira · 自然', gender: '女声' },
  { id: 'es-ES-AlvaroNeural', lang: 'es', label: 'Alvaro · 沉稳', gender: '男声' },
  { id: 'ru-RU-SvetlanaNeural', lang: 'ru', label: 'Svetlana · 自然', gender: '女声' },
  { id: 'ru-RU-DmitryNeural', lang: 'ru', label: 'Dmitry · 沉稳', gender: '男声' },
  { id: 'it-IT-ElsaNeural', lang: 'it', label: 'Elsa · 自然', gender: '女声' },
  { id: 'it-IT-DiegoNeural', lang: 'it', label: 'Diego · 沉稳', gender: '男声' },
  { id: 'pt-BR-FranciscaNeural', lang: 'pt', label: 'Francisca · 自然', gender: '女声' },
  { id: 'pt-BR-AntonioNeural', lang: 'pt', label: 'Antonio · 沉稳', gender: '男声' },
  { id: 'ar-EG-SalmaNeural', lang: 'ar', label: 'Salma · 自然', gender: '女声' },
  { id: 'ar-EG-ShakirNeural', lang: 'ar', label: 'Shakir · 沉稳', gender: '男声' },
  { id: 'th-TH-PremwadeeNeural', lang: 'th', label: 'Premwadee · 自然', gender: '女声' },
  { id: 'th-TH-NiwatNeural', lang: 'th', label: 'Niwat · 沉稳', gender: '男声' },
];

export function edgeVoicesFor(target: string): EdgeVoice[] {
  return EDGE_VOICES.filter(v => v.lang === target);
}

export function edgeVoiceFor(target: string, selected: string): string {
  const voices = edgeVoicesFor(target);
  return voices.find(v => v.id === selected)?.id ?? voices[0]?.id ?? '';
}

export function speakSample(target: string): string {
  const samples: Record<string, string> = {
    zh: '你好，世界。今天过得怎么样？',
    en: 'Hello, world. How are you today?',
    ja: 'こんにちは、世界。今日はどうですか。',
    ko: '안녕하세요, 세계. 오늘 어떻게 지내세요?',
    fr: 'Bonjour le monde. Comment allez-vous ?',
    de: 'Hallo Welt. Wie geht es dir heute?',
    es: 'Hola, mundo. ¿Cómo estás hoy?',
    ru: 'Привет, мир. Как ты сегодня?',
    it: 'Ciao, mondo. Come stai oggi?',
    pt: 'Olá, mundo. Como você está hoje?',
    ar: 'مرحبًا بالعالم. كيف حالك اليوم؟',
    th: 'สวัสดีชาวโลก วันนี้เป็นอย่างไรบ้าง',
  };
  return samples[target] ?? samples.en;
}
