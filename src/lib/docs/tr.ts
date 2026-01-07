// Turkish documentation
export const documentation = `# Jottery Dokumantasyonu

## Icindekiler

- [Baslangic](#baslangic)
- [Not Olusturma ve Duzenleme](#not-olusturma-ve-duzenleme)
- [Soz Dizimi Vurgulama](#soz-dizimi-vurgulama)
- [Hesap Makinesi Modu](#hesap-makinesi-modu)
- [Arama](#arama)
  - [Temel Arama](#temel-arama)
  - [Etiket Aramasi](#etiket-aramasi)
  - [Gelismis Arama Degistiricileri](#gelismis-arama-degistiricileri)
- [Coklu Secim ve Toplu Islemler](#coklu-secim-ve-toplu-islemler)
- [Surum Gecmisi](#surum-gecmisi)
- [Klavye Kisayollari](#klavye-kisayollari)
- [Senkronizasyon](#senkronizasyon)
- [Guvenlik ve Gizlilik](#guvenlik-ve-gizlilik)
- [Iceri ve Disari Aktarma](#iceri-ve-disari-aktarma)

---

## Baslangic

Jottery, gizlilige odaklanan, sifreli bir not alma uygulamasidir. Tum notlariniz yerel olarak **AES-256-GCM** sifreleme kullanilarak saklanmadan once sifrelenir.

> **Onemli:** Sifreniz, sifreleme anahtaridir. Eger kaybederseniz, notlariniz kurtarilamaz. Sifre sifirlama ozelligi bulunmamaktadir.

---

## Not Olusturma ve Duzenleme

| Islem | Nasil yapilir |
|-------|---------------|
| **Not olusturma** | "+ Yeni Not" dügmesine tiklayin veya \`Alt+N\` tusuna basin |
| **Not duzenleme** | Acmak icin listedeki bir nota tiklayin |
| **Otomatik kaydetme** | Siz yazarken degisiklikler otomatik olarak kaydedilir |
| **Not kapatma** | \`Escape\` tusuna basin veya baska bir nota tiklayin |
| **Not sabitleme** | Notu en ustte tutmak icin sabitleme simgesine tiklayin |
| **Not silme** | Menüye (⋮) tiklayin ve "Sil" secenegini secin |

---

## Soz Dizimi Vurgulama

Soz dizimi vurgulamayi etkinlestirmek icin düzenleyicideki dil acilir menusunu kullanin. Desteklenen diller sunlardir:

- **Markdown** - canli onizleme ve kod blogu vurgulama ile
- **JavaScript/TypeScript** - ES6+ soz dizimi destegi
- **Python** - f-string'ler ve decorator'lar dahil
- **JSON, HTML, CSS, SQL**
- **Bash/Shell, Perl**
- **Calculator** - etkilesimli matematik ifadeleri

---

## Hesap Makinesi Modu

Etkilesimli hesap makinesini kullanmak icin soz dizimi dilini **Calc** olarak ayarlayin. Her satir matematiksel bir ifade olarak degerlendirilir ve sonuclar satir icinde gosterilir.

### Ozellikler

- **Temel aritmetik:** \`2 + 3 * 4\` → \`14\`
- **Degiskenler:** \`x = 10\` ardindan \`x * 2\` → \`20\`
- **Sabitler:** \`pi\`, \`e\`, \`tau\`, \`phi\`
- **Fonksiyonlar:** \`sqrt(16)\` → \`4\`, \`sin(pi/2)\` → \`1\`
- **Us alma:** \`2^10\` veya \`2**10\` → \`1024\`
- **Faktoriyel:** \`5!\` → \`120\`
- **Yorumlar:** \`#\` ile baslayan satirlar yok sayilir

### Mevcut Fonksiyonlar

| Kategori | Fonksiyonlar |
|----------|--------------|
| **Temel** | \`abs\`, \`floor\`, \`ceil\`, \`round\`, \`min\`, \`max\` |
| **Usler** | \`sqrt\`, \`cbrt\`, \`exp\`, \`ln\`, \`log\`, \`log10\` |
| **Trigonometri** | \`sin\`, \`cos\`, \`tan\`, \`asin\`, \`acos\`, \`atan\` |
| **Hiperbolik** | \`sinh\`, \`cosh\`, \`tanh\`, \`asinh\`, \`acosh\`, \`atanh\` |

### Ornek

\`\`\`
# Bilesik faiz hesapla
principal = 1000
rate = 0.05
years = 10
principal * (1 + rate)^years
\`\`\`

---

## Arama

### Temel Arama

Notlari bulmak icin arama kutusuna yazin. Arama hem not icerigine hem de etiketlere bakar.

| Soz Dizimi | Aciklama |
|------------|----------|
| \`kelime\` | "kelime" iceren notlar |
| \`kelime1 kelime2\` | Her iki kelimeyi de iceren notlar (VE) |
| \`"tam ifade"\` | Tam ifadeyi iceren notlar |
| \`-kelime\` | "kelime" iceren notlari haric tut |

### Etiket Aramasi

| Soz Dizimi | Aciklama |
|------------|----------|
| \`#etiketadi\` | Bu etikete sahip notlar |
| \`#etiket1 #etiket2\` | Her iki etikete de sahip notlar (VE) |
| \`#etiket1 \\| #etiket2\` | Etiketlerden birine sahip notlar (VEYA) |

### Gelismis Arama Degistiricileri

| Degistirici | Aciklama | Ornek |
|-------------|----------|-------|
| \`has:attachment\` | Eki olan notlar | \`has:attachment\` |
| \`created:>TARIH\` | Tarihten sonra olusturulan | \`created:>2024-01-01\` |
| \`created:<TARIH\` | Tarihten once olusturulan | \`created:<2024-06-30\` |
| \`created:TARIH..TARIH\` | Tarih araliginda olusturulan | \`created:2024-01-01..2024-06-30\` |
| \`modified:>TARIH\` | Tarihten sonra degistirilen | \`modified:>2024-01-01\` |
| \`modified:<TARIH\` | Tarihten once degistirilen | \`modified:<2024-06-30\` |
| \`words:>N\` | N'den fazla kelime | \`words:>100\` |
| \`words:<N\` | N'den az kelime | \`words:<50\` |
| \`words:N..M\` | Kelime sayisi aralikta | \`words:50..200\` |

**Degistiricileri birlestirme:** \`#proje has:attachment modified:>2024-01-01 words:>100\`

---

## Coklu Secim ve Toplu Islemler

Toplu islemler gerceklestirmek icin birden fazla not secin.

### Not Secme

| Islem | Nasil yapilir |
|-------|---------------|
| **Secimi degistir** | Bir nota \`Ctrl/Cmd + Tikla\` |
| **Aralik secimi** | Son secilenden secmek icin \`Shift + Tikla\` |
| **Tum gorunenleri sec** | Arac cubugundan "Tumunu Sec"e tiklayin |
| **Secimi temizle** | \`Escape\` tusuna basin veya "Iptal"e tiklayin |

### Toplu Islemler

Notlar secildiginde, altta su seceneklerle bir arac cubugu belirir:

- **Etiket Ekle** - Tum secili notlara etiket ekle
- **Etiket Kaldir** - Secili notlardan belirli etiketleri kaldir
- **Disari Aktar** - Secili notlari JSON olarak disari aktar
- **Birlestir** - Secili notlari tek bir notta birlestir (olusturma tarihine gore siralanir)
- **Sil** - Secili notlari geri donusum kutusuna tasi

---

## Surum Gecmisi

Jottery, notlari senkronize ederken otomatik olarak surum anlık goruntuleri olusturur.

| Islem | Nasil yapilir |
|-------|---------------|
| **Gecmisi ac** | ⋮ menusune tiklayin → "Surum Gecmisi" veya \`Alt+H\` tusuna basin |
| **Surumu goruntule** | Icerigini gormek icin bir surume tiklayin |
| **Karsilastir** | Farkliliklar otomatik olarak vurgulanir |
| **Geri yukle** | Onceki bir surume donmek icin "Geri Yukle"ye tiklayin |

---

## Klavye Kisayollari

Tum klavye kisayollari Ayarlar → Klavye Kisayollari bolumunden ozellestirilebilir.

### Varsayilan Kisayollar

| Kisayol | Islem |
|---------|-------|
| \`Ctrl/Cmd + K\` | Aramaya odaklan |
| \`Alt + N\` | Yeni not olustur |
| \`Ctrl/Cmd + Z\` | Geri al |
| \`Ctrl/Cmd + Shift + Z\` | Yinele |
| \`Alt + H\` | Surum gecmisi |
| \`Alt + I\` | Not bilgisi |
| \`Escape\` | Notu kapat / Secimi temizle |
| \`Ctrl/Cmd + ,\` | Ayarlari ac |

### Coklu Secim Kisayollari

| Kisayol | Islem |
|---------|-------|
| \`Ctrl/Cmd + Tikla\` | Not secimini degistir |
| \`Shift + Tikla\` | Aralik secimi |
| \`Ctrl/Cmd + A\` | Filtrelenmis tum notlari sec |

---

## Senkronizasyon

Jottery, cihazlar arasi kendi kendine barindirilan senkronizasyonu destekler.

### Kurulum

1. **Ayarlar → Senkronizasyon** bolumune gidin
2. Kendi kendine barindirilan sunucu URL'nizi girin
3. **Ilk cihaz:** Senkronizasyon kimlik bilgileri olusturmak icin "Cihaz Kaydet"e tiklayin
4. **Diger cihazlar:** Senkronizasyon kimlik bilgilerinizle "Mevcut Kimlik Bilgilerini Kullan"i kullanin

> **Onemli:** Tum cihazlar notlarin sifresini cozmek icin **ayni sifreyi** kullanmalidir. Sifre asla sunucuya gonderilmez.

### Nasil Calisir

- Notlar cihaznizdan **ayrilmadan once** sifrelenir
- Sunucu yalnizca sifreli verileri saklar
- Cevrimici oldugunuzda senkronizasyon otomatik olarak gerceklesir
- Catismalar son-yazma-kazanir yontemiyle cozulur

---

## Guvenlik ve Gizlilik

| Ozellik | Aciklama |
|---------|----------|
| **Sifreleme** | Tum not icerigi ve etiketler icin AES-256-GCM |
| **Yerel sifreleme** | Tum sifreleme tarayicinizda gerceklesir |
| **Sifre** | Asla saklanmaz veya iletilmez |
| **Otomatik kilitleme** | Bosta iken notlari korur (varsayilan: 15 dakika) |
| **Izleme yok** | Sifir analitik veya ucuncu taraf betikleri |
| **Acik kaynak** | Tam kaynak kodu GitHub'da mevcuttur |

> **Ipucu:** Jottery icin guclu ve benzersiz bir sifre olusturmak ve saklamak icin bir sifre yoneticisi kullanin. Sifre kurtarma olmadigi icin, sifrenizi kaybetmek notlariniza erisimi kalici olarak kaybetmeniz anlamina gelir.

### Sifrenizi Degistirme

Sifreniz sifreleme anahtari oldugu icin, dogrudan degistirmenin bir yolu yoktur. Ancak sifrenizi etkili bir sekilde degistirebilirsiniz:

1. Tum notlarinizi **disari aktarin** (Ayarlar → Iceri/Disari Aktarma → Disari Aktar)
2. Yerel verilerinizi **temizleyin** veya yeni bir tarayici/cihaz kullanin
3. Jottery'yi yeni sifrenizle **kurun**
4. Disari aktarilan notlarinizi **iceri aktarin**

Notlariniz yeni sifre ile yeniden sifrelenir.

---

## Iceri ve Disari Aktarma

### Disari Aktarma

1. **Ayarlar → Iceri/Disari Aktarma** bolumune gidin
2. "Tum Notlari Disari Aktar"a tiklayin
3. JSON dosyasini kaydetmek icin bir konum secin

> **Uyari:** Disari aktarimlar **sifrelenmemistir**. Guvenli bir sekilde saklayin!

### Iceri Aktarma

1. **Ayarlar → Iceri/Disari Aktarma** bolumune gidin
2. "Notlari Iceri Aktar"a tiklayin
3. Daha once disari aktarilmis bir JSON dosyasi secin
4. Notlar mevcut verilerle birlestirilir (kopyalar atlanir)

### Toplu Disari Aktarma

Yalnizca secili notlari disari aktarmak icin birden fazla not secin ve "Disari Aktar"a tiklayin.
`;
