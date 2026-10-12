# BIRLIK — dunyodagi har qanday qurilmani bitta AI kuchiga aylantirish

> **Oʻzbekcha oʻqish nusxasi (12.10.2026, v3).** Loyihaning asosiy hujjatlari endi ingliz tilida:
> [PLAN.md](../../PLAN.md) va [REQUIREMENTS.md](../../REQUIREMENTS.md). Bu nusxa siz oʻqishingiz uchun saqlangan.
> Loyihadagi fayl va token nomlari ham inglizchaga oʻtdi: `dizayn/` → `design/`, `konsepsiya.html` → `concept.html`,
> `tirik-halqa.js` → `living-ring.js`, `ranglar.py` → `colors.py`, `tokenlar.*` → `tokens.*`, `manba/` → `upstream/`;
> tokenlar: `--fon` → `--bg`, `--yuza-N` → `--surface-N`, `--matn-N` → `--text-N`, `--firuza` → `--turquoise`,
> `--oltin` → `--gold`, `--oltin-belgi` → `--gold-mark`, `--lojuvard` → `--lapis`, `--yuklama-N` → `--load-N`.
> Interfeysning asosiy tili — ingliz tili; oʻzbekcha (lotin, kirill) va ruscha — tarjima.

> Ishchi nom. Ochiq loyiha, Apache-2.0. Versiya 3 (12.10.2026): texnologiya, dizayn va UI/UX talablari qo'shildi.
> Manba kodlar `manba/` ichida. Ishlatiladigan kodlarning litsenziyalari Apache-2.0 bilan mos (jadval 7-bo'limda).
> **Qanday qilinadi** (arxitektura, texnologiyalar, dizayn tizimi, bosqichlar) — [REJA.md](REJA.md).

## 1. G'oya

Bugun dunyoda milliardlab telefon, kompyuter, planshet va televizor kunning ko'p qismida bo'sh turadi.
Ularning umumiy hisoblash kuchi har qanday ma'lumot markazidan katta.
**BIRLIK — shu bo'sh kuchni bitta AI tarmog'iga yig'adigan ochiq tizim.**

U uch miqyosda ishlaydi:

| Miqyos | Kim | Misol |
|---|---|---|
| **Shaxsiy** | o'z qurilmalaringiz | uydagi 2 kompyuter + 3 telefon = 70B model |
| **Tashkilot** | idora, universitet, vazirlik | 500 ta ish kompyuteri tunda bitta suveren AI klasterga aylanadi |
| **Ommaviy** | dunyo bo'ylab ixtiyoriy ishtirokchilar | qurilma ulagan odam evaziga tarmoqdan foydalanish krediti oladi |

Ishlatilishi: inference (javob berish), o'qitish (fine-tuning, RL) va har xil modellar (matn, nutq, rasm, embedding).

## 2. Jahon darajasidagi raqiblar va ularning chegarasi

| Tizim | Kuchi | Chegarasi |
|---|---|---|
| exo | Mac klasteri, RDMA, tensor parallel | faqat Mac; telefon, Windows yo'q; faqat LAN |
| prima.cpp | kuchsiz qurilmalarda 70B, Wi-Fi, disk | faqat CUDA; qurilmalar oldindan qo'lda yoziladi; Android faqat Termux orqali |
| GPUStack | NVIDIA serverlarni boshqarish | faqat Linux server; telefon, Mac yo'q |
| Parallax (Gradient) | internet orqali turli GPU, 2 bosqichli rejalashtiruvchi | faqat GPU va Mac; telefon va brauzer yo'q |
| Petals | BitTorrent uslubidagi ommaviy tarmoq | 2024 yildan beri to'xtagan; faqat GPU |
| distributed-llama | CPU'da tensor parallel | faqat bir xil qurilmalar, LAN |
| Prime Intellect / Psyche | internet orqali o'qitish | faqat ma'lumot markazi GPU'lari, inference emas |

**Hech biri bir vaqtda uchta narsani qila olmaydi: istalgan qurilma (telefon, brauzer, NPU), istalgan miqyos
(uydan dunyogacha) va inference bilan o'qitish birgalikda.** BIRLIK shu uchalasini birlashtiradi.

## 3. Jahon darajasiga olib chiqadigan 8 ta yechim

### Y1. Qurilma ko'paygan sari bitta javob ham tezlashadi
Oddiy pipeline'da token qurilmalardan navbat bilan o'tadi, qolgan qurilmalar kutib turadi.
**Yechim:** uzluksiz pipeline + taxminiy dekodlash. Har bir qurilma bir paytning o'zida boshqa taxminiy
token ustida ishlaydi va pipeline bo'sh turmaydi. Ilmiy natijalar: PipeSpec — 2,54× gacha tezlanish va
pipeline chuqurligi oshgan sari tezlik o'sib boradi; FlowSpec — 1,36–1,77×; DiP-SD (2026).
Sekin qurilmalar uchun katta taxmin daraxtlari ishlatiladi (SpecExec usuli): bitta tekshiruvda 10–20 token qabul qilinadi.

### Y2. Faqat faol qismni hisoblash
- **MoE ekspertlari qurilmalarga tarqatiladi.** DeepSeek V3 (671B) har tokenga atigi 37B'ni, Kimi K2 (1T)
  esa 32B'ni ishlatadi. Shuning uchun 1 trillion parametrli model ko'plab oddiy qurilmada ishlaydi.
  Keyingi qatlamda kerak bo'ladigan ekspertlar oldindan taxmin qilinadi va tayyorlab qo'yiladi.
- **«Issiq» va «sovuq» neyronlar ajratiladi** (PowerInfer, MIT). Tez-tez ishlaydigan neyronlar tez
  videokartada, kam ishlaydiganlari sekin qurilmada turadi.

### Y3. Qurilmadagi har bir hisoblash bloki ishlaydi
Bitta qurilmada CPU, GPU va NPU bir vaqtda ishlaydi: Qualcomm Hexagon, Apple Neural Engine, Intel/AMD NPU.
Uchala raqibning hech biri NPU ishlatmaydi. NPU faqat aniqlik darvozasidan o'tgan amallarda ishlaydi:
har amal natijasi CPU natijasi bilan solishtiriladi, farq chegaradan oshsa o'sha amal GPU yoki CPU'ga o'tadi.
(llama.cpp Hexagon backend'i hozir tajriba bosqichida, ba'zi modellarda noto'g'ri chiqish beradi —
darvoza aynan shuning uchun kerak.)

### Y4. Hech narsa o'rnatmasdan qo'shilish
QR kodni skanerlash kifoya: brauzer oynasi WebGPU orqali klaster tuguniga aylanadi (llama.cpp WASM + ggml-webgpu —
boshqa qurilmalar bilan bir xil GGUF formati).
Shu yo'l bilan istalgan noutbuk, planshet, televizor yoki boshqa birovning telefoni o'rnatishsiz qo'shiladi.

### Y5. Bitta faollashuv protokoli, ko'p dvigatel
Bir pipeline ichida turli dvigatellar birga ishlaydi: ggml (CUDA, Vulkan, Metal, OpenCL, CPU), MLX,
WebGPU, vLLM/SGLang. Ular o'zaro bitta standart protokol bilan gaplashadi: yashirin holat tenzori
(fp16/fp8), pozitsiya va so'rov identifikatori. Har bir qurilma bitta modelning bitta nusxasidan o'z
formatiga o'girilgan qatlamlarni oladi. Kuchli qurilmaga Q8, kuchsiz telefonga Q4 berilishi mumkin.

### Y6. Har qanday aloqa, siqilgan trafik
- **Aloqa turlari:** Wi-Fi, Ethernet, USB-C kabel (telefon↔kompyuter: kechikishi Wi-Fi'dan past va barqaror;
  o'tkazuvchanligi telefon modeliga bog'liq, ulanganda o'lchanadi), Thunderbolt/RDMA, Wi-Fi Direct.
- **Bir nechta yo'ldan birga:** Wi-Fi va USB bir vaqtda ishlatiladi.
- **Siqish:** faollashuvlar fp8/int8 qilib yuboriladi, trafik 2–4 marta kamayadi.
- **Markazsiz boshqaruv:** yagona boshliq yo'q, qurilmalar DHT orqali bir-birini topadi. Shu sababli
  minglab tugunga yetadi. Internet orqali ishlaydi: QUIC, NAT teshish, teshib bo'lmasa rele.

### Y7. Uzilmaydigan tizim
- Har bir qatlam bo'lagining zaxirasi boshqa qurilmada turadi.
- KV kesh qo'shni qurilmaga doimiy nusxalanib boradi. Qurilma chiqib ketsa, javob boshidan qayta
  hisoblanmaydi: zaxira 1 soniya ichida o'sha joydan davom ettiradi.
- Prefiks kesh butun klaster uchun umumiy (Mooncake usuli, Apache-2.0).

### Y8. O'qitish ham shu tarmoqda
- **LoRA fine-tuning** — DiLoCo usulida: har tugun ~500 qadam mustaqil o'qiydi, keyin faqat siqilgan
  farqni yuboradi. Oddiy internet yetadi.
- **RL rollout'lar** (INTELLECT-2 usuli): inference tugunlari o'z tezligida javob yaratadi, o'qituvchi
  tugun ulardan o'rganadi.
- **Chegara:** noldan to'liq pretraining maqsad emas, chunki u ma'lumot markazi tarmog'ini talab qiladi.
- **Kim o'qitadi:** o'qitish faqat PyTorch ishlaydigan GPU tugunlarida bajariladi (CUDA, ROCm, Apple MPS).
  Telefon va brauzer tugunlari RL uchun javob yaratadi (inference).

### Y9. Uzun kontekst ko'p qurilma xotirasida
Telefonlarda KV kesh tez to'ladi. Juda uzun matnlar uchun ketma-ketlik qurilmalarga bo'linadi (ring attention).
Shunda kontekst uzunligi qurilmalar soniga qarab o'sadi.

## 4. Ishonch va maxfiylik (ommaviy tarmoq uchun qarorlar)

- **Xom matn tarmoqqa chiqmaydi.** Ommaviy tarmoqda birinchi va oxirgi qatlamlar (embedding va chiqish)
  doim so'rov egasining qurilmasida bajariladi.
- **Maxfiy so'rov faqat shaxsiy yoki tashkilot klasterida.** Yashirin holatdan matnni qisman tiklash
  mumkin, shuning uchun to'liq maxfiylik faqat o'sha klasterlarda bo'ladi. Bu qoida panelda ochiq yozib qo'yiladi.
- **Halollik tekshiruvi.** Har bir javob bilan TOPLOC isboti keladi: oxirgi yashirin holatning top-k qiymatlari.
  Bu isbot qayta hisoblashdan ~100 marta arzon tekshiriladi va model, prompt yoki aniqlik almashtirilganini
  ushlaydi. Qo'shimcha ravishda oraliq qatlamlar ishining 1–2 % tasodifiy boshqa tugunda qayta hisoblanadi
  (farqli apparat uchun ruxsat etilgan chegara bilan). Mos kelmasa, tugun reytingi tushadi; takrorlansa,
  tarmoqdan chiqariladi.
- **Kredit tizimi.** Qurilma bergan hisoblash (token×qatlam) hisoblanadi va shunga teng foydalanish
  krediti beriladi. Blokcheyn ishlatilmaydi: kvitansiyalar Ed25519 bilan imzolanadi va shaffoflik jurnaliga
  (C2SP tlog-tiles) yoziladi. Jurnalni har kim tekshira oladi.
- **Shifrlash.** Barcha trafik shifrlanadi. Qurilma klasterga QR kod yoki bir martalik kod bilan qo'shiladi.

## 5. Raqamlangan talablar

**Dvigatel (D)**
- D1 — prima.cpp yadrosi (piped-ring, oldindan o'qish, mmap) hozirgi llama.cpp'ga ko'chiriladi.
- D2 — Y5 faollashuv protokoli; ggml, MLX, WebGPU va vLLM/SGLang adapterlari.
- D3 — Y1: uzluksiz pipeline + taxminiy dekodlash + katta taxmin daraxtlari.
- D4 — Y2: MoE ekspertlarini tarqatish va oldindan taxmin qilish; issiq/sovuq neyronlar.
- D5 — Y3: CPU+GPU+NPU birga ishlashi va aniqlik darvozasi.
- D6 — Dinamik batching, prefill/dekod ajratish (exo `disaggregated`), har qurilmaga o'z kvantlashi.
- D7 — Matndan tashqari modellar: STT, TTS, rasm, embedding. Kichik modellar telefonlarga butunligicha beriladi.

**Tarmoq (T)**
- T1 — Tugun agenti — bitta Rust ikkilik fayl: Linux, Windows, macOS, Android, iOS.
- T2 — DHT orqali topish (LAN'da mDNS ham), QUIC, NAT teshish, rele.
- T3 — Y6: barcha aloqa turlari, bir nechta yo'ldan birga uzatish, fp8/int8 siqish.
- T4 — Har bir havolaning kechikishi va o'tkazuvchanligi jonli o'lchanadi.
- T5 — Model P2P tarqatiladi: har qurilma faqat o'z bo'lagini oladi, internetdan bir marta yuklanadi.

**Rejalashtiruvchi (R)**
- R1 — Halda (prima, ILP) va Parallax'ning ikki bosqichli rejalashtiruvchisi asosida. Hisobga olinadi:
  xotira, hisoblash, disk, havola, batareya, qizish, energiya (joul/token).
- R2 — Qurilma ish paytida qo'shilsa yoki chiqsa, reja to'xtamasdan qayta tuziladi.
- R3 — Y7: zaxira qatlamlar, KV kesh nusxasi, ≤ 1 s ichida davom etish.
- R4 — Rejim so'rovga qarab avtomatik tanlanadi: tezlik / o'tkazuvchanlik / sig'im.
- R5 — Tensor parallel guruhlari faqat ≤ 1 ms havolalarda tuziladi, guruhlar orasida pipeline.

**Qurilmalar (Q)**
- Q1 — Linux, Windows va macOS uchun tray ilova; foydalanuvchi resurs ulushini va ish vaqtini o'zi belgilaydi.
- Q2 — Android (minSdk 26): native ilova, Vulkan/OpenCL/Hexagon.
  Zaryadda ishlaydigan fon xizmati turi: API ≥ 34 da `specialUse`, API < 34 da `dataSync`.
  Sababi: Android 15'dan boshlab `dataSync` sutkasiga 6 soatdan keyin to'xtatiladi.
- Q3 — iOS (17+): native ilova, Metal/ANE. iOS fonda uzoq hisoblashga ruxsat bermaydi, shuning uchun
  «Tun rejimi» bor: telefon zaryadda, ilova ochiq, ekran deyarli qora.
- Q4 — Brauzer tuguni: llama.cpp (WASM) + WebGPU, QR orqali, o'rnatishsiz.
  WebGPU faqat HTTPS sahifada ishlaydi, shuning uchun sahifa loyiha domenidan yuklanib keshda qoladi.
  Internet umuman yo'q tarmoqda brauzer tuguni faqat oldin keshlangan bo'lsa ishlaydi.
- Q5 — Batareya va qizish chegarasi: qurilma yukini zaxiraga bo'shatib beradi.

**Miqyos (M)**
- K1 — Shaxsiy klaster.
- K2 — Tashkilot klasteri: ish vaqti jadvali, Active Directory/LDAP orqali kirish, markaziy siyosat.
- K3 — Ommaviy tarmoq: 4-bo'limdagi ishonch, maxfiylik va kredit qoidalari.

**O'qitish (O)**
- O1 — LoRA fine-tuning (DiLoCo / OpenDiLoCo usuli).
- O2 — RL rollout'lari (prime-rl usuli).

**API va panel (A)**
- A1 — OpenAI, Claude Messages va Ollama API'lariga mos (exo `api/adapters`).
- A2 — Jonli dunyo xaritasi: tugunlar, oqimlar, har qurilmadagi qatlamlar, energiya.
- A3 — Foydalanuvchilar, kalitlar, token hisobi, Prometheus (GPUStack).

**UI/UX (U)** — batafsil [REJA.md §9](REJA.md)
- U1 — «Halqa» bosh ekrani. Har qurilma — bitta yoy; yoy uzunligi qatlamlar ulushiga teng, rangi joriy yuklamani
  bildiradi. Token halqa bo'ylab o'lchangan haqiqiy tezlikda yuradi, taxminiy tokenlar ham ko'rinadi.
- U2 — Bitta dizayn tizimi: tokenlar bitta JSON'dan (W3C DTCG) CSS, Swift va Kotlin'ga yig'iladi.
  Ikki mavzu bor (tun — standart, kun). Urg'u rangi bitta — firuza.
- U3 — Shriftlar: Inter + IBM Plex Mono, o'zbek lotin (U+02BB) va kirill belgilari to'liq. Shrift shkalasi
  9 pog'onali, boshqa o'lcham taqiqlangan.
- U4 — WCAG 2.2 AA: barcha matn ≥ 4,5:1; holat rangi doim belgi va yozuv bilan; grafik ranglari rang ko'rligi
  tekshiruvidan o'tgan; har grafikning jadval ko'rinishi bor; butun ilova klaviatura bilan boshqariladi.
- U5 — Harakat faqat `transform`/`opacity` bilan; kam harakat rejimida zarrachalar o'chadi.
- U6 — Qurilma qo'shish ≤ 30 s (QR + 4 emoji tasdig'i); birinchi ishga tushirish ≤ 60 s.
- U7 — ⌘K buyruqlar paneli va har amal uchun klaviatura yorlig'i.
- U8 — Platforma tajribasi: tray'dagi halqa; iOS Live Activity, Dynamic Island, vidjet, Siri; Android
  Quick Settings plitkasi, Glance vidjet; iOS Tun rejimi.
- U9 — Tillar: o'zbekcha (lotin, kirill), ruscha, inglizcha. Sana va raqamlar o'z formatlovchimiz bilan
  formatlanadi, chunki Chrome ICU'da o'zbekcha nomlar yo'q.
- U11 — Tirik belgi (WebGL2 zarrachali halqa) tizim mexanizmini jonli ko'rsatadi: yoy — qurilma, oltin kometa — token,
  oraliq — tarmoq. Kirish sahnasi, 3D og'ish va qurilma qo'shish ta'siri bor. Kam harakat rejimida statik, WebGL2
  bo'lmasa SVG chiqadi. Batafsil — [REJA.md §9.3](REJA.md).
- U10 — Byudjetlar:
  - panelning boshlang'ich JS hajmi ≤ 180 KB, brauzer tuguni sahifasi ≤ 60 KB;
  - 1000 qurilmada 60 fps;
  - mobil ilova sovuq start ≤ 1 s.

## 6. Jahon darajasi mezonlari (ochiq o'lchanadi)

| # | Mezon |
|---|---|
| W1 | Bitta so'rov tezligi qurilma soni oshgan sari o'sadi (2 → 4 → 8 → 16 qurilmada o'lchanadi) |
| W2 | Bir xil apparatda 70B modelda exo, prima.cpp, Parallax va distributed-llama'dan tezroq |
| W3 | 1T MoE (Kimi K2 sinfi) xotirasi ≤ 32 GB bo'lgan oddiy qurilmalar to'plamida ishlaydi |
| W4 | 6 xil tugun turi: Linux, Windows, macOS, Android, iOS, brauzer |
| W5 | Hisoblash bloklari: CUDA, ROCm/Vulkan, Metal, OpenCL, NPU |
| W6 | Ish paytida tugun uzilsa, ≤ 1 s ichida qayta hisoblashsiz davom etadi |
| W7 | Ommaviy tarmoq: 1000+ tugun (konteynerlarda, haqiqiy protokol bilan) + kamida 20 ta real qurilma |
| W8 | Brauzerdan QR orqali qo'shilish ≤ 30 s |
| W9 | NVIDIA serverda o'tkazuvchanlik vLLM'nikidan kam emas |
| W10 | Barcha o'lchov skriptlari ochiq; istalgan odam natijani takrorlay oladi |

## 7. Ishlatiladigan ochiq kodlar

| Loyiha | Litsenziya | Nima olinadi |
|---|---|---|
| prima.cpp (OpenCPIL) | MIT | piped-ring, Halda, mmap/oldindan o'qish |
| llama.cpp / ggml | MIT | asosiy dvigatel, barcha backend'lar, Hexagon |
| exo | Apache-2.0 | topish, joylashtirish, API adapterlari, panel, MLX, disaggregated |
| GPUStack | Apache-2.0 | foydalanuvchilar, monitoring, vLLM/SGLang boshqaruvi |
| Parallax | Apache-2.0 | internet miqyosidagi rejalashtiruvchi, KV boshqaruvi |
| PowerInfer | MIT | issiq/sovuq neyronlar |
| Petals / hivemind | MIT | DHT, ommaviy tarmoq, tugun reytingi |
| web-llm | Apache-2.0 | faqat solishtirish uchun (brauzer tuguni — llama.cpp WebGPU) |
| distributed-llama | MIT | CPU tensor parallel |
| Mooncake | Apache-2.0 | taqsimlangan KV kesh |
| prime-rl / OpenDiLoCo | Apache-2.0 | o'qitish |

## 8. Bosqichlar

B0–B9. Har bosqichning ishi va darvozasi [REJA.md §11](REJA.md) da yozilgan. UI har bosqich bilan birga
quriladi (U1–U10), oxirga qoldirilmaydi.

| B | Talablar |
|---|---|
| B0 Poydevor | U2, U3, U4 (dizayn tizimi, komponentlar), CI, protokol v1 |
| B1 Dvigatel | D1, D2 |
| B2 Tarmoq | T1–T5, U1, U6 |
| B3 Rejalashtiruvchi | R1–R5 |
| B4 Tezlik | D3, D4, D6, Y9 |
| B5 Qurilmalar | Q1–Q5, D5, U8, U10 |
| B6 Miqyos | K1–K3 |
| B7 Modallik va o'qitish | D7, O1, O2 |
| B8 Orollar va API | A1–A3, U7 |
| B9 Jahon o'lchovi | W1–W10, U9 |

## 9. GitHub

- Repo: `HumoyunDavronov2001/birlik` (nom tasdiqlangach).
- `NOTICE` faylida 7-bo'limdagi barcha mualliflar ko'rsatiladi.
- Har bosqich alohida PR. CI'da Linux, Windows, macOS va Android yig'ilishi hamda o'lchov sinovlari.
