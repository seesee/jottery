// Portuguese (Brazilian) documentation
export const documentation = `# Documentação do Jottery

## Índice

- [Primeiros Passos](#primeiros-passos)
- [Criando e Editando Notas](#criando-editando-notas)
- [Realce de Sintaxe](#realce-de-sintaxe)
- [Modo Calculadora](#modo-calculadora)
- [Pesquisa](#pesquisa)
  - [Pesquisa Básica](#pesquisa-basica)
  - [Pesquisa por Tags](#pesquisa-por-tags)
  - [Modificadores de Pesquisa Avançada](#modificadores-de-pesquisa-avancada)
- [Seleção Múltipla e Operações em Lote](#selecao-multipla-operacoes-em-lote)
- [Histórico de Versões](#historico-de-versoes)
- [Atalhos de Teclado](#atalhos-de-teclado)
- [Sincronização](#sincronizacao)
- [Segurança e Privacidade](#seguranca-privacidade)
- [Importar e Exportar](#importar-exportar)

---

## Primeiros Passos

Jottery é um aplicativo de anotações criptografado e focado em privacidade. Todas as suas notas são criptografadas localmente usando criptografia **AES-256-GCM** antes de serem armazenadas.

> **Importante:** Sua senha é a chave de criptografia. Se você perdê-la, suas notas não poderão ser recuperadas. Não existe funcionalidade de redefinição de senha.

---

## Criando e Editando Notas

| Ação | Como fazer |
|------|------------|
| **Criar uma nota** | Clique em "+ Nova Nota" ou pressione \`Alt+N\` |
| **Editar uma nota** | Clique em uma nota na lista para abri-la |
| **Salvamento automático** | As alterações são salvas automaticamente enquanto você digita |
| **Fechar uma nota** | Pressione \`Escape\` ou clique em outra nota |
| **Fixar uma nota** | Clique no ícone de alfinete para mantê-la no topo |
| **Excluir uma nota** | Clique no menu (⋮) e selecione "Excluir" |

---

## Realce de Sintaxe

Use o menu suspenso de idioma no editor para ativar o realce de sintaxe. As linguagens suportadas incluem:

- **Markdown** - com visualização ao vivo e realce de blocos de código
- **JavaScript/TypeScript** - suporte à sintaxe ES6+
- **Python** - incluindo f-strings e decoradores
- **JSON, HTML, CSS, SQL**
- **Bash/Shell, Perl**
- **Calculator** - expressões matemáticas interativas

---

## Modo Calculadora

Defina a linguagem de sintaxe como **Calc** para usar a calculadora interativa. Cada linha é avaliada como uma expressão matemática, com resultados exibidos inline.

### Recursos

- **Aritmética básica:** \`2 + 3 * 4\` → \`14\`
- **Variáveis:** \`x = 10\` depois \`x * 2\` → \`20\`
- **Constantes:** \`pi\`, \`e\`, \`tau\`, \`phi\`
- **Funções:** \`sqrt(16)\` → \`4\`, \`sin(pi/2)\` → \`1\`
- **Potência:** \`2^10\` ou \`2**10\` → \`1024\`
- **Fatorial:** \`5!\` → \`120\`
- **Comentários:** Linhas começando com \`#\` são ignoradas

### Funções Disponíveis

| Categoria | Funções |
|-----------|---------|
| **Básicas** | \`abs\`, \`floor\`, \`ceil\`, \`round\`, \`min\`, \`max\` |
| **Potências** | \`sqrt\`, \`cbrt\`, \`exp\`, \`ln\`, \`log\`, \`log10\` |
| **Trigonometria** | \`sin\`, \`cos\`, \`tan\`, \`asin\`, \`acos\`, \`atan\` |
| **Hiperbólicas** | \`sinh\`, \`cosh\`, \`tanh\`, \`asinh\`, \`acosh\`, \`atanh\` |

### Exemplo

\`\`\`
# Calcular juros compostos
principal = 1000
rate = 0.05
years = 10
principal * (1 + rate)^years
\`\`\`

---

## Pesquisa

### Pesquisa Básica

Digite na caixa de pesquisa para encontrar notas. A pesquisa procura tanto no conteúdo das notas quanto nas tags.

| Sintaxe | Descrição |
|---------|-----------|
| \`palavra\` | Notas contendo "palavra" |
| \`palavra1 palavra2\` | Notas contendo ambas as palavras (E) |
| \`"frase exata"\` | Notas contendo a frase exata |
| \`-palavra\` | Excluir notas contendo "palavra" |

### Pesquisa por Tags

| Sintaxe | Descrição |
|---------|-----------|
| \`#nometag\` | Notas com esta tag |
| \`#tag1 #tag2\` | Notas com ambas as tags (E) |
| \`#tag1 \\| #tag2\` | Notas com qualquer uma das tags (OU) |

### Modificadores de Pesquisa Avançada

| Modificador | Descrição | Exemplo |
|-------------|-----------|---------|
| \`has:attachment\` | Notas com anexos | \`has:attachment\` |
| \`created:>DATA\` | Criadas após a data | \`created:>2024-01-01\` |
| \`created:<DATA\` | Criadas antes da data | \`created:<2024-06-30\` |
| \`created:DATA..DATA\` | Criadas no intervalo de datas | \`created:2024-01-01..2024-06-30\` |
| \`modified:>DATA\` | Modificadas após a data | \`modified:>2024-01-01\` |
| \`modified:<DATA\` | Modificadas antes da data | \`modified:<2024-06-30\` |
| \`words:>N\` | Mais de N palavras | \`words:>100\` |
| \`words:<N\` | Menos de N palavras | \`words:<50\` |
| \`words:N..M\` | Contagem de palavras no intervalo | \`words:50..200\` |

**Combinando modificadores:** \`#projeto has:attachment modified:>2024-01-01 words:>100\`

---

## Seleção Múltipla e Operações em Lote

Selecione várias notas para realizar ações em lote.

### Selecionando Notas

| Ação | Como fazer |
|------|------------|
| **Alternar seleção** | \`Ctrl/Cmd + Clique\` em uma nota |
| **Seleção de intervalo** | \`Shift + Clique\` para selecionar a partir da última selecionada |
| **Selecionar todas visíveis** | Clique em "Selecionar Tudo" na barra de ferramentas |
| **Limpar seleção** | Pressione \`Escape\` ou clique em "Cancelar" |

### Ações em Lote

Quando notas são selecionadas, uma barra de ferramentas aparece na parte inferior com estas opções:

- **Adicionar Tags** - Adicionar tags a todas as notas selecionadas
- **Remover Tags** - Remover tags específicas das notas selecionadas
- **Exportar** - Exportar notas selecionadas como JSON
- **Combinar** - Mesclar notas selecionadas em uma (ordenadas por data de criação)
- **Excluir** - Mover notas selecionadas para a lixeira

---

## Histórico de Versões

O Jottery cria automaticamente snapshots de versão ao sincronizar notas.

| Ação | Como fazer |
|------|------------|
| **Abrir histórico** | Clique no menu ⋮ → "Histórico de Versões" ou pressione \`Alt+H\` |
| **Visualizar versão** | Clique em uma versão para ver seu conteúdo |
| **Comparar** | Diferenças são destacadas automaticamente |
| **Restaurar** | Clique em "Restaurar" para reverter para uma versão anterior |

---

## Atalhos de Teclado

Todos os atalhos de teclado são personalizáveis em Configurações → Atalhos de Teclado.

### Atalhos Padrão

| Atalho | Ação |
|--------|------|
| \`Ctrl/Cmd + K\` | Focar na pesquisa |
| \`Alt + N\` | Criar nova nota |
| \`Ctrl/Cmd + Z\` | Desfazer |
| \`Ctrl/Cmd + Shift + Z\` | Refazer |
| \`Alt + H\` | Histórico de versões |
| \`Alt + I\` | Informações da nota |
| \`Escape\` | Fechar nota / Limpar seleção |
| \`Ctrl/Cmd + ,\` | Abrir configurações |

### Atalhos de Seleção Múltipla

| Atalho | Ação |
|--------|------|
| \`Ctrl/Cmd + Clique\` | Alternar seleção de nota |
| \`Shift + Clique\` | Seleção de intervalo |
| \`Ctrl/Cmd + A\` | Selecionar todas as notas filtradas |

---

## Sincronização

O Jottery suporta sincronização auto-hospedada entre dispositivos.

### Configuração

1. Vá para **Configurações → Sincronização**
2. Digite a URL do seu servidor auto-hospedado
3. **Primeiro dispositivo:** Clique em "Registrar Dispositivo" para criar credenciais de sincronização
4. **Outros dispositivos:** Use "Usar Credenciais Existentes" com suas credenciais de sincronização

> **Importante:** Todos os dispositivos devem usar a **mesma senha** para descriptografar as notas. A senha nunca é enviada ao servidor.

### Como Funciona

- As notas são criptografadas **antes** de sair do seu dispositivo
- O servidor armazena apenas dados criptografados
- A sincronização acontece automaticamente quando online
- Conflitos são resolvidos usando a última gravação vence

---

## Segurança e Privacidade

| Recurso | Descrição |
|---------|-----------|
| **Criptografia** | AES-256-GCM para todo o conteúdo das notas e tags |
| **Criptografia local** | Toda a criptografia acontece no seu navegador |
| **Senha** | Nunca armazenada ou transmitida |
| **Bloqueio automático** | Protege as notas quando inativo (padrão: 15 minutos) |
| **Sem rastreamento** | Zero análises ou scripts de terceiros |
| **Código aberto** | Código-fonte completo disponível no GitHub |

> **Dica:** Use um gerenciador de senhas para gerar e armazenar uma senha forte e única para o Jottery. Como não há recuperação de senha, perder sua senha significa perder acesso às suas notas permanentemente.

### Alterando Sua Senha

Como sua senha é a chave de criptografia, não há uma maneira direta de alterá-la. No entanto, você pode efetivamente mudar sua senha:

1. **Exporte** todas as suas notas (Configurações → Importar/Exportar → Exportar)
2. **Limpe** seus dados locais ou use um novo navegador/dispositivo
3. **Configure** o Jottery com sua nova senha
4. **Importe** suas notas exportadas

Suas notas serão recriptografadas com a nova senha.

---

## Importar e Exportar

### Exportar

1. Vá para **Configurações → Importar/Exportar**
2. Clique em "Exportar Todas as Notas"
3. Escolha um local para salvar o arquivo JSON

> **Aviso:** Exportações são **não criptografadas**. Armazene-as com segurança!

### Importar

1. Vá para **Configurações → Importar/Exportar**
2. Clique em "Importar Notas"
3. Selecione um arquivo JSON previamente exportado
4. As notas serão mescladas com os dados existentes (duplicatas são ignoradas)

### Exportação em Lote

Selecione várias notas e clique em "Exportar" para exportar apenas as notas selecionadas.
`;
