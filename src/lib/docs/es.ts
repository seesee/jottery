// Spanish documentation
export const documentation = `# Documentación de Jottery

## Tabla de Contenidos

- [Primeros Pasos](#primeros-pasos)
- [Crear y Editar Notas](#crear-y-editar-notas)
- [Resaltado de Sintaxis](#resaltado-de-sintaxis)
- [Modo Calculadora](#modo-calculadora)
- [Búsqueda](#busqueda)
  - [Búsqueda Básica](#busqueda-basica)
  - [Búsqueda por Etiquetas](#busqueda-por-etiquetas)
  - [Modificadores de Búsqueda Avanzada](#modificadores-de-busqueda-avanzada)
- [Selección Múltiple y Operaciones Masivas](#seleccion-multiple-y-operaciones-masivas)
- [Historial de Versiones](#historial-de-versiones)
- [Atajos de Teclado](#atajos-de-teclado)
- [Sincronización](#sincronizacion)
- [Seguridad y Privacidad](#seguridad-y-privacidad)
- [Importar y Exportar](#importar-y-exportar)

---

## Primeros Pasos

Jottery es una aplicación de notas cifradas centrada en la privacidad. Todas sus notas se cifran localmente utilizando cifrado **AES-256-GCM** antes de almacenarse.

> **Importante:** Su contraseña es la clave de cifrado. Si la pierde, sus notas no podrán recuperarse. No existe funcionalidad de recuperación de contraseña.

---

## Crear y Editar Notas

| Acción | Cómo hacerlo |
|--------|--------------|
| **Crear una nota** | Haga clic en "+ Nueva Nota" o presione \`Alt+N\` |
| **Editar una nota** | Haga clic en una nota de la lista para abrirla |
| **Guardado automático** | Los cambios se guardan automáticamente mientras escribe |
| **Cerrar una nota** | Presione \`Escape\` o haga clic en otra nota |
| **Fijar una nota** | Haga clic en el icono de pin para mantenerla en la parte superior |
| **Eliminar una nota** | Haga clic en el menú (⋮) y seleccione "Eliminar" |

---

## Resaltado de Sintaxis

Utilice el menú desplegable de idioma en el editor para activar el resaltado de sintaxis. Los lenguajes compatibles incluyen:

- **Markdown** - con vista previa en vivo y resaltado de bloques de código
- **JavaScript/TypeScript** - soporte de sintaxis ES6+
- **Python** - incluyendo f-strings y decoradores
- **JSON, HTML, CSS, SQL**
- **Bash/Shell, Perl**
- **Calculadora** - expresiones matemáticas interactivas

---

## Modo Calculadora

Configure el lenguaje de sintaxis en **Calc** para usar la calculadora interactiva. Cada línea se evalúa como una expresión matemática, con los resultados mostrados en línea.

### Características

- **Aritmética básica:** \`2 + 3 * 4\` → \`14\`
- **Variables:** \`x = 10\` luego \`x * 2\` → \`20\`
- **Constantes:** \`pi\`, \`e\`, \`tau\`, \`phi\`
- **Funciones:** \`sqrt(16)\` → \`4\`, \`sin(pi/2)\` → \`1\`
- **Potencia:** \`2^10\` o \`2**10\` → \`1024\`
- **Factorial:** \`5!\` → \`120\`
- **Comentarios:** Las líneas que comienzan con \`#\` se ignoran

### Funciones Disponibles

| Categoría | Funciones |
|-----------|-----------|
| **Básicas** | \`abs\`, \`floor\`, \`ceil\`, \`round\`, \`min\`, \`max\` |
| **Potencias** | \`sqrt\`, \`cbrt\`, \`exp\`, \`ln\`, \`log\`, \`log10\` |
| **Trigonometría** | \`sin\`, \`cos\`, \`tan\`, \`asin\`, \`acos\`, \`atan\` |
| **Hiperbólicas** | \`sinh\`, \`cosh\`, \`tanh\`, \`asinh\`, \`acosh\`, \`atanh\` |

### Ejemplo

\`\`\`
# Calcular interés compuesto
principal = 1000
rate = 0.05
years = 10
principal * (1 + rate)^years
\`\`\`

---

## Búsqueda

### Búsqueda Básica

Escriba en el cuadro de búsqueda para encontrar notas. La búsqueda examina tanto el contenido de las notas como las etiquetas.

| Sintaxis | Descripción |
|----------|-------------|
| \`palabra\` | Notas que contienen "palabra" |
| \`palabra1 palabra2\` | Notas que contienen ambas palabras (Y) |
| \`"frase exacta"\` | Notas que contienen la frase exacta |
| \`-palabra\` | Excluir notas que contienen "palabra" |

### Búsqueda por Etiquetas

| Sintaxis | Descripción |
|----------|-------------|
| \`#etiqueta\` | Notas con esta etiqueta |
| \`#etiqueta1 #etiqueta2\` | Notas con ambas etiquetas (Y) |
| \`#etiqueta1 \\| #etiqueta2\` | Notas con cualquiera de las etiquetas (O) |

### Modificadores de Búsqueda Avanzada

| Modificador | Descripción | Ejemplo |
|-------------|-------------|---------|
| \`has:attachment\` | Notas con archivos adjuntos | \`has:attachment\` |
| \`created:>FECHA\` | Creadas después de la fecha | \`created:>2024-01-01\` |
| \`created:<FECHA\` | Creadas antes de la fecha | \`created:<2024-06-30\` |
| \`created:FECHA..FECHA\` | Creadas en el rango de fechas | \`created:2024-01-01..2024-06-30\` |
| \`modified:>FECHA\` | Modificadas después de la fecha | \`modified:>2024-01-01\` |
| \`modified:<FECHA\` | Modificadas antes de la fecha | \`modified:<2024-06-30\` |
| \`words:>N\` | Más de N palabras | \`words:>100\` |
| \`words:<N\` | Menos de N palabras | \`words:<50\` |
| \`words:N..M\` | Cantidad de palabras en el rango | \`words:50..200\` |

**Combinando modificadores:** \`#proyecto has:attachment modified:>2024-01-01 words:>100\`

---

## Selección Múltiple y Operaciones Masivas

Seleccione múltiples notas para realizar acciones masivas.

### Seleccionar Notas

| Acción | Cómo hacerlo |
|--------|--------------|
| **Alternar selección** | \`Ctrl/Cmd + Clic\` en una nota |
| **Selección por rango** | \`Shift + Clic\` para seleccionar desde la última seleccionada |
| **Seleccionar todas las visibles** | Haga clic en "Seleccionar Todo" en la barra de herramientas |
| **Limpiar selección** | Presione \`Escape\` o haga clic en "Cancelar" |

### Acciones Masivas

Cuando hay notas seleccionadas, aparece una barra de herramientas en la parte inferior con estas opciones:

- **Agregar Etiquetas** - Agregar etiquetas a todas las notas seleccionadas
- **Eliminar Etiquetas** - Eliminar etiquetas específicas de las notas seleccionadas
- **Exportar** - Exportar las notas seleccionadas como JSON
- **Combinar** - Fusionar las notas seleccionadas en una sola (ordenadas por fecha de creación)
- **Eliminar** - Mover las notas seleccionadas a la papelera de reciclaje

---

## Historial de Versiones

Jottery crea automáticamente instantáneas de versiones al sincronizar notas.

| Acción | Cómo hacerlo |
|--------|--------------|
| **Abrir historial** | Haga clic en el menú ⋮ → "Historial de Versiones" o presione \`Alt+H\` |
| **Ver versión** | Haga clic en una versión para ver su contenido |
| **Comparar** | Las diferencias se resaltan automáticamente |
| **Restaurar** | Haga clic en "Restaurar" para volver a una versión anterior |

---

## Atajos de Teclado

Todos los atajos de teclado son personalizables en Configuración → Atajos de Teclado.

### Atajos Predeterminados

| Atajo | Acción |
|-------|--------|
| \`Ctrl/Cmd + K\` | Enfocar búsqueda |
| \`Alt + N\` | Crear nueva nota |
| \`Ctrl/Cmd + Z\` | Deshacer |
| \`Ctrl/Cmd + Shift + Z\` | Rehacer |
| \`Alt + H\` | Historial de versiones |
| \`Alt + I\` | Información de la nota |
| \`Escape\` | Cerrar nota / Limpiar selección |
| \`Ctrl/Cmd + ,\` | Abrir configuración |

### Atajos de Selección Múltiple

| Atajo | Acción |
|-------|--------|
| \`Ctrl/Cmd + Clic\` | Alternar selección de nota |
| \`Shift + Clic\` | Selección por rango |
| \`Ctrl/Cmd + A\` | Seleccionar todas las notas filtradas |

---

## Sincronización

Jottery admite sincronización autoalojada entre dispositivos.

### Configuración

1. Vaya a **Configuración → Sincronización**
2. Ingrese la URL de su servidor autoalojado
3. **Primer dispositivo:** Haga clic en "Registrar Dispositivo" para crear credenciales de sincronización
4. **Otros dispositivos:** Utilice "Usar Credenciales Existentes" con sus credenciales de sincronización

> **Importante:** Todos los dispositivos deben usar la **misma contraseña** para descifrar las notas. La contraseña nunca se envía al servidor.

### Cómo Funciona

- Las notas se cifran **antes** de salir de su dispositivo
- El servidor solo almacena datos cifrados
- La sincronización ocurre automáticamente cuando está en línea
- Los conflictos se resuelven usando la última escritura gana

---

## Seguridad y Privacidad

| Característica | Descripción |
|----------------|-------------|
| **Cifrado** | AES-256-GCM para todo el contenido de notas y etiquetas |
| **Cifrado local** | Todo el cifrado ocurre en su navegador |
| **Contraseña** | Nunca se almacena ni se transmite |
| **Bloqueo automático** | Protege las notas cuando está inactivo (predeterminado: 15 minutos) |
| **Sin rastreo** | Cero análisis o scripts de terceros |
| **Código abierto** | Código fuente completo disponible en GitHub |

> **Consejo:** Utilice un gestor de contraseñas para generar y almacenar una contraseña fuerte y única para Jottery. Dado que no hay recuperación de contraseña, perder su contraseña significa perder acceso a sus notas permanentemente.

### Cambiar Su Contraseña

Dado que su contraseña es la clave de cifrado, no hay una forma directa de cambiarla. Sin embargo, puede cambiar efectivamente su contraseña mediante:

1. **Exportar** todas sus notas (Configuración → Importar/Exportar → Exportar)
2. **Borrar** sus datos locales o usar un nuevo navegador/dispositivo
3. **Configurar** Jottery con su nueva contraseña
4. **Importar** sus notas exportadas

Sus notas se volverán a cifrar con la nueva contraseña.

---

## Importar y Exportar

### Exportar

1. Vaya a **Configuración → Importar/Exportar**
2. Haga clic en "Exportar Todas las Notas"
3. Elija una ubicación para guardar el archivo JSON

> **Advertencia:** Las exportaciones están **sin cifrar**. ¡Almacénelas de forma segura!

### Importar

1. Vaya a **Configuración → Importar/Exportar**
2. Haga clic en "Importar Notas"
3. Seleccione un archivo JSON previamente exportado
4. Las notas se fusionarán con los datos existentes (los duplicados se omiten)

### Exportación Masiva

Seleccione múltiples notas y haga clic en "Exportar" para exportar solo las notas seleccionadas.
`;
